//! The OnePass server.
//!
//! How one request travels:
//!   1. A company asks OnePass for facts about a person (e.g. "is a student").
//!   2. The person gets a notification and approves or denies. They also see,
//!      and can choose, which of their institutions each fact will come from.
//!   3. Only after approval, OnePass fetches each fact from that institution.
//!   4. OnePass checks the institution's signature and checks on the Stellar
//!      registry that the institution is genuine. If not, it refuses.
//!   5. OnePass stamps each fact with its own signature ("verified by OnePass")
//!      and hands it to the company. The institution's name is not passed on.
//!   6. The company checks OnePass's stamp against OnePass's key on Stellar.
//!   7. OnePass records a fingerprint of what it vouched for on Stellar, as a
//!      permanent receipt, and forgets the facts.
//!
//! It also serves the web pages, including the invented demo companies.
//! Everything is kept in memory, so restarting the server resets the demo.

mod company;
mod directory;
mod registry;

use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use directory::SourceInfo;
use onepass_core::{fingerprint, sign_fact, stellar_address, verify_fact, SignedFact, SigningKey};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

/// Where the stand-in institutions are running.
const SOURCES_URL: &str = "http://127.0.0.1:4001";

/// The receipt OnePass leaves on Stellar for one completed request.
#[derive(Clone, Serialize)]
struct Receipt {
    /// Fingerprint of what OnePass vouched for (reveals nothing personal).
    fingerprint: String,
    /// "pending", "recorded", or "not recorded"
    status: String,
    detail: Option<String>,
}

/// A company's request, from creation to the final outcome.
#[derive(Clone, Serialize)]
struct Request {
    id: String,
    company: company::Company,
    /// "pending"  = waiting for the person to answer
    /// "verified" = shared, and the company accepted it
    /// "rejected" = shared, but the company's checks failed
    /// "refused"  = OnePass itself would not vouch (e.g. untrusted institution)
    /// "denied"   = the person said no
    status: String,
    /// Why OnePass refused, in words the person can read.
    reason: Option<String>,
    /// What the company received: facts stamped by OnePass. No institution names.
    delivered: Vec<SignedFact>,
    verdict: Option<company::Verdict>,
    receipt: Option<Receipt>,
}

/// Everything the server remembers while it runs.
struct AppState {
    client: reqwest::Client,
    /// The trusted list on Stellar (or a local fallback).
    registry: registry::Registry,
    /// OnePass's own signing key, used to stamp facts it has verified.
    onepass_key: SigningKey,
    /// institution id -> access token. This is ALL OnePass keeps about the person's accounts.
    connections: Mutex<HashMap<String, String>>,
    requests: Mutex<HashMap<String, Request>>,
    /// Request ids in the order they arrived, for the notification panel.
    order: Mutex<Vec<String>>,
}

type Shared = Arc<AppState>;
type ApiError = (StatusCode, Json<Value>);

fn error(status: StatusCode, message: impl Into<String>) -> ApiError {
    (status, Json(json!({ "error": message.into() })))
}

/// Which institutions exist and what each can provide (not a source of trust).
async fn directory(state: &AppState) -> Result<Vec<SourceInfo>, ApiError> {
    directory::load(&state.client, SOURCES_URL)
        .await
        .map_err(|e| error(StatusCode::BAD_GATEWAY, e))
}

fn random_id() -> String {
    let mut bytes = [0u8; 8];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

// ───────────────────────── Connecting institutions ─────────────────────────

/// GET /api/sources — every institution, whether the person has connected it,
/// and whether it is on the registry.
async fn list_sources(State(state): State<Shared>) -> Result<Json<Value>, ApiError> {
    let sources = directory(&state).await?;
    let trusted = state.registry.trusted().await;
    let connections = state.connections.lock().unwrap();
    let list: Vec<Value> = sources
        .iter()
        .map(|s| {
            let registered = trusted.as_ref().ok().map(|t| t.iter().any(|x| x.public_key == s.public_key));
            json!({
                "id": s.id, "name": s.name, "kind": s.kind, "public_key": s.public_key,
                "fields": s.fields, "connected": connections.contains_key(&s.id),
                "registered": registered,
            })
        })
        .collect();
    let onepass_key = stellar_address(&state.onepass_key.verifying_key());
    let onepass_registered = trusted.as_ref().ok().map(|t| {
        t.iter().any(|x| x.public_key == onepass_key && x.kind == company::VERIFIER_KIND)
    });
    Ok(Json(json!({
        "sources": list,
        "registry": state.registry.describe(),
        "registry_url": state.registry.explorer_url(),
        "registry_error": trusted.err(),
        "onepass_key": onepass_key,
        "onepass_registered": onepass_registered,
    })))
}

#[derive(Deserialize)]
struct Connect {
    source_id: String,
    username: String,
    password: String,
}

/// POST /api/connect — sign in to an institution on the person's behalf.
/// The password is passed straight through and never saved; only the token is kept.
async fn connect(State(state): State<Shared>, Json(body): Json<Connect>) -> Result<Json<Value>, ApiError> {
    let response = state
        .client
        .post(format!("{SOURCES_URL}/sources/{}/login", body.source_id))
        .json(&json!({ "username": body.username, "password": body.password }))
        .send()
        .await
        .map_err(|_| error(StatusCode::BAD_GATEWAY, "Could not reach the institution"))?;
    if !response.status().is_success() {
        return Err(error(StatusCode::UNAUTHORIZED, "Wrong username or password"));
    }
    let reply: Value = response.json().await.map_err(|e| error(StatusCode::BAD_GATEWAY, e.to_string()))?;
    let token = reply["token"].as_str().unwrap_or_default().to_string();
    state.connections.lock().unwrap().insert(body.source_id, token);
    Ok(Json(json!({ "connected": true })))
}

#[derive(Deserialize)]
struct Disconnect {
    source_id: String,
}

/// POST /api/disconnect — forget the token, so OnePass can no longer fetch from there.
async fn disconnect(State(state): State<Shared>, Json(body): Json<Disconnect>) -> Json<Value> {
    state.connections.lock().unwrap().remove(&body.source_id);
    Json(json!({ "connected": false }))
}

// ───────────────────────────── The company's side ─────────────────────────────

/// GET /api/companies — the demo companies and what each asks for.
async fn list_companies() -> Json<Value> {
    Json(json!({ "companies": company::companies() }))
}

#[derive(Deserialize)]
struct NewRequest {
    company_id: String,
}

/// POST /api/requests — a company asks for facts about the person.
/// Nothing is fetched yet. The request just waits in the person's notifications.
async fn create_request(State(state): State<Shared>, Json(body): Json<NewRequest>) -> Result<Json<Value>, ApiError> {
    let company = company::companies()
        .into_iter()
        .find(|c| c.id == body.company_id)
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "No such company"))?;
    let request = Request {
        id: random_id(),
        company,
        status: "pending".into(),
        reason: None,
        delivered: Vec::new(),
        verdict: None,
        receipt: None,
    };
    let id = request.id.clone();
    state.requests.lock().unwrap().insert(id.clone(), request);
    state.order.lock().unwrap().push(id.clone());
    Ok(Json(json!({ "id": id })))
}

/// GET /api/requests/{id} — what the COMPANY can see about its request.
/// There is deliberately nothing here about which institution a fact came from.
async fn get_request(State(state): State<Shared>, Path(id): Path<String>) -> Result<Json<Request>, ApiError> {
    state
        .requests
        .lock()
        .unwrap()
        .get(&id)
        .cloned()
        .map(Json)
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "No such request"))
}

// ───────────────────────────── The person's side ─────────────────────────────

/// GET /api/notifications — the person's requests, newest first.
/// For each pending one, it lists which connected institutions could supply
/// each fact, so the person can see and choose the source.
async fn notifications(State(state): State<Shared>) -> Result<Json<Value>, ApiError> {
    let sources = directory(&state).await?;
    let trusted = state.registry.trusted().await.unwrap_or_default();
    let connections = state.connections.lock().unwrap().clone();
    let order = state.order.lock().unwrap().clone();
    let requests = state.requests.lock().unwrap();

    let list: Vec<Value> = order
        .iter()
        .rev()
        .filter_map(|id| requests.get(id))
        .map(|r| {
            let items: Vec<Value> = r
                .company
                .wants
                .iter()
                .map(|field| {
                    let options: Vec<Value> = sources
                        .iter()
                        .filter(|s| connections.contains_key(&s.id) && s.fields.iter().any(|f| f == field))
                        .map(|s| {
                            json!({
                                "id": s.id, "name": s.name,
                                "registered": trusted.iter().any(|t| t.public_key == s.public_key),
                            })
                        })
                        .collect();
                    json!({ "field": field, "options": options })
                })
                .collect();
            json!({
                "id": r.id, "company": r.company.name, "status": r.status,
                "reason": r.reason, "items": items,
            })
        })
        .collect();
    let pending = list.iter().filter(|n| n["status"] == "pending").count();
    Ok(Json(json!({ "pending": pending, "notifications": list })))
}

#[derive(Deserialize)]
struct Approval {
    /// fact name -> id of the institution the person chose to supply it.
    /// A fact that is left out is simply not shared.
    choices: HashMap<String, String>,
    /// Demo switch: alter a fact after OnePass stamps it, to show the company catches it.
    #[serde(default)]
    tamper: bool,
}

/// Mark a request as refused by OnePass and say why.
fn refuse(state: &AppState, id: &str, reason: String) -> Result<Json<Request>, ApiError> {
    let mut requests = state.requests.lock().unwrap();
    let request = requests.get_mut(id).ok_or_else(|| error(StatusCode::NOT_FOUND, "No such request"))?;
    request.status = "refused".to_string();
    request.reason = Some(reason);
    Ok(Json(request.clone()))
}

/// POST /api/requests/{id}/approve — the person says yes.
/// This is where OnePass does its real work: fetch, verify, stamp, deliver.
async fn approve(
    State(state): State<Shared>,
    Path(id): Path<String>,
    Json(body): Json<Approval>,
) -> Result<Json<Request>, ApiError> {
    let wants = {
        let requests = state.requests.lock().unwrap();
        let request = requests.get(&id).ok_or_else(|| error(StatusCode::NOT_FOUND, "No such request"))?;
        if request.status != "pending" {
            return Err(error(StatusCode::CONFLICT, "This request was already answered"));
        }
        request.company.wants.clone()
    };

    let sources = directory(&state).await?;
    let trusted = state.registry.trusted().await.map_err(|e| error(StatusCode::BAD_GATEWAY, e))?;
    let connections = state.connections.lock().unwrap().clone();

    let mut delivered: Vec<SignedFact> = Vec::new();
    for field in wants.iter().filter(|f| body.choices.contains_key(**f)) {
        let source_id = &body.choices[*field];
        let Some(source) = sources.iter().find(|s| &s.id == source_id) else {
            return refuse(&state, &id, format!("Unknown institution '{source_id}'."));
        };
        let Some(token) = connections.get(source_id) else {
            return refuse(&state, &id, format!("{} is not connected.", source.name));
        };

        // Step 3: fetch this one fact from the institution the person chose.
        let reply: Value = state
            .client
            .get(format!("{SOURCES_URL}/sources/{}/facts?fields={}", source.id, field))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|_| error(StatusCode::BAD_GATEWAY, "Could not reach the institution"))?
            .json()
            .await
            .map_err(|e| error(StatusCode::BAD_GATEWAY, e.to_string()))?;
        let Ok(original) = serde_json::from_value::<SignedFact>(reply["facts"][0].clone()) else {
            return refuse(&state, &id, format!("{} would not provide this. Try reconnecting it.", source.name));
        };

        // Step 4: OnePass verifies before it vouches.
        if !verify_fact(&original) {
            return refuse(&state, &id, format!("{}'s signature did not check out.", source.name));
        }
        if !trusted.iter().any(|t| t.public_key == original.source_key) {
            return refuse(
                &state,
                &id,
                format!("{} is not on the Stellar registry of trusted institutions, so OnePass will not vouch for it.", source.name),
            );
        }

        // Step 5: stamp it with OnePass's own signature. The institution's
        // signed original is dropped here; the company never sees it.
        delivered.push(sign_fact(&state.onepass_key, field, &original.value, now()));
    }

    // Step 7 (prepared now, sent below): a fingerprint of exactly what was vouched for.
    let receipt_fingerprint = fingerprint(&delivered);

    if body.tamper {
        if let Some(fact) = delivered.first_mut() {
            fact.value = match &fact.value {
                Value::Bool(b) => json!(!b),
                _ => json!("Someone Else"),
            };
        }
    }

    // Step 6: the company runs its own checks on what arrived.
    let verdict = company::verify(
        &wants,
        &delivered,
        &trusted,
        state.registry.describe(),
        state.registry.explorer_url(),
    );

    let snapshot = {
        let mut requests = state.requests.lock().unwrap();
        let request = requests.get_mut(&id).ok_or_else(|| error(StatusCode::NOT_FOUND, "No such request"))?;
        request.status = if verdict.ok { "verified" } else { "rejected" }.to_string();
        request.verdict = Some(verdict);
        request.delivered = delivered;
        request.receipt = Some(Receipt {
            fingerprint: receipt_fingerprint.clone(),
            status: "pending".to_string(),
            detail: None,
        });
        request.clone()
    };

    // Step 7: write the receipt to Stellar in the background (it takes a few
    // seconds), so the person and the company are not kept waiting.
    let background = state.clone();
    tokio::spawn(async move {
        let outcome = background.registry.record_receipt(&receipt_fingerprint).await;
        let mut requests = background.requests.lock().unwrap();
        if let Some(receipt) = requests.get_mut(&id).and_then(|r| r.receipt.as_mut()) {
            match outcome {
                Ok(()) => receipt.status = "recorded".to_string(),
                Err(why) => {
                    receipt.status = "not recorded".to_string();
                    receipt.detail = Some(why);
                }
            }
        }
    });

    Ok(Json(snapshot))
}

/// POST /api/requests/{id}/deny — the person says no. Nothing is fetched or shared.
async fn deny(State(state): State<Shared>, Path(id): Path<String>) -> Result<Json<Request>, ApiError> {
    let mut requests = state.requests.lock().unwrap();
    let request = requests.get_mut(&id).ok_or_else(|| error(StatusCode::NOT_FOUND, "No such request"))?;
    if request.status == "pending" {
        request.status = "denied".to_string();
    }
    Ok(Json(request.clone()))
}

// ───────────────────────────────── Web pages ─────────────────────────────────
// The pages are plain HTML files in server/static, baked into the program.

async fn home_page() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}
async fn company_page() -> Html<&'static str> {
    Html(include_str!("../static/company.html"))
}
async fn style() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/css")], include_str!("../static/style.css"))
}
async fn script() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../static/app.js"))
}

#[tokio::main]
async fn main() {
    let state: Shared = Arc::new(AppState {
        client: reqwest::Client::new(),
        registry: registry::Registry::detect(),
        // DEMO ONLY: a fixed key, so the same key can be listed on Stellar by the setup script.
        onepass_key: onepass_core::demo_key("onepass"),
        connections: Mutex::new(HashMap::new()),
        requests: Mutex::new(HashMap::new()),
        order: Mutex::new(Vec::new()),
    });

    let app = Router::new()
        .route("/", get(home_page))
        .route("/company/{id}", get(company_page))
        .route("/style.css", get(style))
        .route("/app.js", get(script))
        .route("/api/sources", get(list_sources))
        .route("/api/connect", post(connect))
        .route("/api/disconnect", post(disconnect))
        .route("/api/companies", get(list_companies))
        .route("/api/requests", post(create_request))
        .route("/api/requests/{id}", get(get_request))
        .route("/api/requests/{id}/approve", post(approve))
        .route("/api/requests/{id}/deny", post(deny))
        .route("/api/notifications", get(notifications))
        .with_state(state.clone());

    let address = "127.0.0.1:4000";
    let listener = tokio::net::TcpListener::bind(address).await.expect("port 4000 is already in use");
    println!("OnePass is running. Open http://{address} in your browser.");
    println!("Registry: {}", state.registry.describe());
    println!("OnePass verifier key: {}", stellar_address(&state.onepass_key.verifying_key()));
    println!("(The stand-in institutions must also be running: cargo run -p onepass-sources)");
    axum::serve(listener, app).await.unwrap();
}

#[cfg(test)]
mod tests {
    #[test]
    fn onepass_key_matches_the_one_listed_on_the_registry() {
        let key = onepass_core::demo_key("onepass");
        assert_eq!(onepass_core::stellar_address(&key.verifying_key()), crate::registry::ONEPASS_KEY);
    }
}
