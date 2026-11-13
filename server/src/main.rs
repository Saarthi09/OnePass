//! The OnePass server.
//!
//! How one request travels:
//!   1. A company fills in the requester page: who it is about, which facts it
//!      needs, and why. A registered company signs the request, and OnePass
//!      checks that signature against the company's key on the Stellar registry.
//!   2. That person gets a notification on their OnePass page and approves or
//!      denies. They also see, and can choose, which of their institutions each
//!      fact will come from.
//!   3. Only after approval, OnePass fetches each fact from that institution.
//!   4. OnePass checks the institution's signature and checks on the Stellar
//!      registry that the institution is genuine. If not, it refuses.
//!   5. OnePass stamps each fact with its own signature ("verified by OnePass")
//!      and hands it to the company. The institution's name is not passed on.
//!   6. The company checks OnePass's stamp against OnePass's key on Stellar.
//!   7. OnePass records a fingerprint of what it vouched for on Stellar, as a
//!      permanent receipt.
//!   8. Thirty seconds after delivery, OnePass wipes its own copy of the facts.
//!
//! Each stamp is made out to one company and one request and expires after ten
//! minutes, so it cannot be passed on or reused.
//!
//! It also serves the web pages: the landing page, the person's page and the
//! requester page. Everything is kept in memory, so restarting resets the demo.

mod company;
mod directory;
mod registry;

use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use directory::SourceInfo;
use onepass_core::{fingerprint, sign_stamp, stellar_address, verify_fact, SignedFact, SigningKey, Stamp};
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

/// How long a OnePass stamp stays valid after it is issued (10 minutes).
/// The company must check it within that time; afterwards it is worthless.
const STAMP_VALID_SECS: u64 = 600;

/// How long OnePass keeps its own copy of delivered facts (30 seconds).
/// Just long enough for the company to collect them; then they are wiped.
const RETENTION_SECS: u64 = 30;

/// The people who have a OnePass identity in the demo: (OnePass ID, name).
/// The ID is also their username at the stand-in institutions.
const USERS: &[(&str, &str)] = &[("alex", "Alex Rivera"), ("priya", "Priya Nair")];

fn user_name(id: &str) -> Option<&'static str> {
    USERS.iter().find(|(user, _)| *user == id).map(|(_, name)| *name)
}

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
/// This is also exactly what the company can see about it.
#[derive(Clone, Serialize)]
struct Request {
    id: String,
    /// The company asking.
    requester: String,
    /// Whether the company proved it is who it says: "verified", "wrong_key",
    /// "unregistered" or "unsigned" (see company::check_requester).
    requester_check: String,
    /// The OnePass ID and name of the person it is about.
    user: String,
    user_name: String,
    /// Why the company wants it. Shown to the person.
    reason: String,
    /// The facts asked for, by name.
    wants: Vec<String>,
    /// "pending"  = waiting for the person to answer
    /// "verified" = shared, and the company accepted it
    /// "rejected" = shared, but the company's checks failed
    /// "refused"  = OnePass itself would not vouch (e.g. untrusted institution)
    /// "denied"   = the person said no
    status: String,
    /// Why OnePass refused. It can name an institution, so it is shown to the
    /// person only and never sent to the company (`skip` keeps it out).
    #[serde(skip)]
    refusal: Option<String>,
    /// What the company received: facts stamped by OnePass for this company and
    /// this request only. No institution names.
    delivered: Vec<Stamp>,
    /// When those stamps stop being valid.
    expires_at: Option<u64>,
    /// True once OnePass has wiped its own copy of the delivered facts.
    purged: bool,
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
    /// (person, institution id) -> access token.
    /// This is ALL OnePass keeps about anyone's accounts.
    connections: Mutex<HashMap<(String, String), String>>,
    requests: Mutex<HashMap<String, Request>>,
    /// Request ids in the order they arrived, for the notification list.
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

/// A registry entry that may vouch for facts: a bank, university and so on.
/// Companies are on the registry too, but only so they can be recognised when
/// they ask. Being listed as a company gives no right to supply facts.
fn is_institution(entry: &registry::TrustedSource) -> bool {
    entry.kind != company::COMPANY_KIND && entry.kind != company::VERIFIER_KIND
}

fn random_id() -> String {
    let mut bytes = [0u8; 8];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

/// The `?user=alex` part of an address.
#[derive(Deserialize)]
struct WhichUser {
    user: String,
}

// ─────────────────────────────── Reference lists ───────────────────────────────

/// GET /api/users — the people with a OnePass identity (demo).
async fn list_users() -> Json<Value> {
    let users: Vec<Value> = USERS.iter().map(|(id, name)| json!({ "id": id, "name": name })).collect();
    Json(json!({ "users": users }))
}

/// GET /api/catalog — the facts a company can ask for, and the shortcut bundles.
async fn catalog() -> Json<Value> {
    Json(json!({ "fields": company::FIELDS, "presets": company::presets() }))
}

// ───────────────────────── Connecting institutions ─────────────────────────

/// GET /api/sources?user=alex — every institution, whether this person has
/// connected it, and whether it is on the registry.
async fn list_sources(State(state): State<Shared>, Query(who): Query<WhichUser>) -> Result<Json<Value>, ApiError> {
    let sources = directory(&state).await?;
    let trusted = state.registry.trusted().await;
    let connections = state.connections.lock().unwrap();
    let list: Vec<Value> = sources
        .iter()
        .map(|s| {
            let registered = trusted.as_ref().ok().map(|t| t.iter().any(|x| x.public_key == s.public_key));
            json!({
                "id": s.id, "name": s.name, "kind": s.kind, "public_key": s.public_key,
                "fields": s.fields,
                "connected": connections.contains_key(&(who.user.clone(), s.id.clone())),
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
    user: String,
    source_id: String,
    password: String,
}

/// POST /api/connect — sign in to an institution on the person's behalf.
/// The password is passed straight through and never saved; only the token is kept.
async fn connect(State(state): State<Shared>, Json(body): Json<Connect>) -> Result<Json<Value>, ApiError> {
    if user_name(&body.user).is_none() {
        return Err(error(StatusCode::NOT_FOUND, "No OnePass user with that ID"));
    }
    let response = state
        .client
        .post(format!("{SOURCES_URL}/sources/{}/login", body.source_id))
        .json(&json!({ "username": body.user, "password": body.password }))
        .send()
        .await
        .map_err(|_| error(StatusCode::BAD_GATEWAY, "Could not reach the institution"))?;
    if !response.status().is_success() {
        return Err(error(StatusCode::UNAUTHORIZED, "That institution has no record for you, or the password is wrong"));
    }
    let reply: Value = response.json().await.map_err(|e| error(StatusCode::BAD_GATEWAY, e.to_string()))?;
    let token = reply["token"].as_str().unwrap_or_default().to_string();
    state.connections.lock().unwrap().insert((body.user, body.source_id), token);
    Ok(Json(json!({ "connected": true })))
}

#[derive(Deserialize)]
struct Disconnect {
    user: String,
    source_id: String,
}

/// POST /api/disconnect — forget the token, so OnePass can no longer fetch from there.
async fn disconnect(State(state): State<Shared>, Json(body): Json<Disconnect>) -> Json<Value> {
    state.connections.lock().unwrap().remove(&(body.user, body.source_id));
    Json(json!({ "connected": false }))
}

// ───────────────────────────── The company's side ─────────────────────────────

#[derive(Deserialize)]
struct NewRequest {
    /// The company's name.
    requester: String,
    /// OnePass ID of the person it is about.
    user: String,
    /// Which facts are wanted.
    fields: Vec<String>,
    /// Why, in the company's words.
    #[serde(default)]
    reason: String,
    /// DEMO: send this as a scammer using the company's name.
    #[serde(default)]
    impostor: bool,
}

/// POST /api/requests — a company asks for facts about a person.
/// Nothing is fetched yet. The request just waits in that person's notifications.
async fn create_request(State(state): State<Shared>, Json(body): Json<NewRequest>) -> Result<Json<Value>, ApiError> {
    let requester = body.requester.trim().to_string();
    if requester.is_empty() {
        return Err(error(StatusCode::BAD_REQUEST, "Enter your company's name"));
    }
    let user = body.user.trim().to_lowercase();
    let Some(name) = user_name(&user) else {
        return Err(error(StatusCode::NOT_FOUND, "No OnePass user with that ID"));
    };
    // Keep only facts OnePass knows about, in a fixed order, without repeats.
    let wants: Vec<String> = company::FIELDS
        .iter()
        .filter(|f| body.fields.iter().any(|asked| asked == *f))
        .map(|f| f.to_string())
        .collect();
    if wants.is_empty() {
        return Err(error(StatusCode::BAD_REQUEST, "Choose at least one item to request"));
    }

    let reason = body.reason.trim().to_string();

    // The company signs its request (the server plays the company's part here;
    // see company::sign_request). Then OnePass checks who is really asking,
    // against the companies listed on the Stellar registry.
    let proof = company::sign_request(&requester, &user, &wants, &reason, now(), body.impostor);
    let trusted = state.registry.trusted().await.unwrap_or_default();
    let requester_check =
        company::check_requester(&requester, &user, &wants, &reason, proof.as_ref(), &trusted, now()).to_string();

    let request = Request {
        id: random_id(),
        requester,
        requester_check,
        user,
        user_name: name.to_string(),
        reason,
        wants,
        status: "pending".into(),
        refusal: None,
        delivered: Vec::new(),
        expires_at: None,
        purged: false,
        verdict: None,
        receipt: None,
    };
    let id = request.id.clone();
    state.requests.lock().unwrap().insert(id.clone(), request);
    state.order.lock().unwrap().push(id.clone());
    Ok(Json(json!({ "id": id })))
}

// ───────────────────── Demo: the registry admin's switch ─────────────────────

/// GET /api/demo/company — is the demo company on the registry right now?
async fn demo_company(State(state): State<Shared>) -> Result<Json<Value>, ApiError> {
    let trusted = state.registry.trusted().await.map_err(|e| error(StatusCode::BAD_GATEWAY, e))?;
    let key = company::demo_company_key();
    let registered = trusted
        .iter()
        .any(|t| t.public_key == key && t.kind == company::COMPANY_KIND && t.name == company::DEMO_COMPANY);
    Ok(Json(json!({
        "name": company::DEMO_COMPANY,
        "registered": registered,
        "on_stellar": state.registry.explorer_url().is_some(),
    })))
}

#[derive(Deserialize)]
struct SetCompany {
    registered: bool,
}

/// POST /api/demo/company — DEMO: put the demo company on the registry, or take
/// it off. This is the registry admin's job (in real use: after checking the
/// company is genuine), not something a company can do for itself. On Stellar
/// it is a real transaction on the smart contract.
async fn set_demo_company(State(state): State<Shared>, Json(body): Json<SetCompany>) -> Result<Json<Value>, ApiError> {
    let key = company::demo_company_key();
    let outcome = if body.registered {
        state.registry.add(&key, company::DEMO_COMPANY, company::COMPANY_KIND).await
    } else {
        state.registry.remove(&key).await
    };
    outcome.map_err(|e| error(StatusCode::BAD_GATEWAY, e))?;
    demo_company(State(state)).await
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

/// GET /api/notifications?user=alex — this person's requests, newest first.
/// For each pending one, it lists which of their connected institutions could
/// supply each fact, so they can see and choose the source.
async fn notifications(State(state): State<Shared>, Query(who): Query<WhichUser>) -> Result<Json<Value>, ApiError> {
    let sources = directory(&state).await?;
    let trusted = state.registry.trusted().await.unwrap_or_default();
    let connections = state.connections.lock().unwrap().clone();
    let order = state.order.lock().unwrap().clone();
    let requests = state.requests.lock().unwrap();

    let list: Vec<Value> = order
        .iter()
        .rev()
        .filter_map(|id| requests.get(id))
        .filter(|r| r.user == who.user)
        .map(|r| {
            let items: Vec<Value> = r
                .wants
                .iter()
                .map(|field| {
                    let options: Vec<Value> = sources
                        .iter()
                        .filter(|s| {
                            connections.contains_key(&(who.user.clone(), s.id.clone()))
                                && s.fields.iter().any(|f| f == field)
                        })
                        .map(|s| {
                            json!({
                                "id": s.id, "name": s.name,
                                "registered": trusted.iter().any(|t| t.public_key == s.public_key && is_institution(t)),
                            })
                        })
                        .collect();
                    json!({ "field": field, "options": options })
                })
                .collect();
            json!({
                "id": r.id, "requester": r.requester, "requester_check": r.requester_check,
                "reason": r.reason, "status": r.status,
                "refusal": r.refusal, "wants": r.wants, "items": items,
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
fn refuse(state: &AppState, id: &str, why: String) -> Result<Json<Value>, ApiError> {
    let mut requests = state.requests.lock().unwrap();
    let request = requests.get_mut(id).ok_or_else(|| error(StatusCode::NOT_FOUND, "No such request"))?;
    request.status = "refused".to_string();
    request.refusal = Some(why);
    Ok(Json(json!({ "status": request.status })))
}

/// POST /api/requests/{id}/approve — the person says yes.
/// This is where OnePass does its real work: fetch, verify, stamp, deliver.
async fn approve(
    State(state): State<Shared>,
    Path(id): Path<String>,
    Json(body): Json<Approval>,
) -> Result<Json<Value>, ApiError> {
    let (wants, user, requester) = {
        let requests = state.requests.lock().unwrap();
        let request = requests.get(&id).ok_or_else(|| error(StatusCode::NOT_FOUND, "No such request"))?;
        if request.status != "pending" {
            return Err(error(StatusCode::CONFLICT, "This request was already answered"));
        }
        (request.wants.clone(), request.user.clone(), request.requester.clone())
    };
    let issued_at = now();
    let expires_at = issued_at + STAMP_VALID_SECS;

    let sources = directory(&state).await?;
    let trusted = state.registry.trusted().await.map_err(|e| error(StatusCode::BAD_GATEWAY, e))?;
    let connections = state.connections.lock().unwrap().clone();

    let mut delivered: Vec<Stamp> = Vec::new();
    for field in wants.iter().filter(|f| body.choices.contains_key(*f)) {
        let source_id = &body.choices[field];
        let Some(source) = sources.iter().find(|s| &s.id == source_id) else {
            return refuse(&state, &id, format!("Unknown institution '{source_id}'."));
        };
        let Some(token) = connections.get(&(user.clone(), source_id.clone())) else {
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
        if !trusted.iter().any(|t| t.public_key == original.source_key && is_institution(t)) {
            return refuse(
                &state,
                &id,
                format!("{} is not on the Stellar registry of trusted institutions, so OnePass will not vouch for it.", source.name),
            );
        }

        // Step 5: stamp it with OnePass's own signature, made out to this
        // company and this request only, with an expiry time. The institution's
        // signed original is dropped here; the company never sees it.
        delivered.push(sign_stamp(
            &state.onepass_key,
            field,
            &original.value,
            &requester,
            &id,
            issued_at,
            expires_at,
        ));
    }

    // Step 7 (prepared now, sent below): a fingerprint of exactly what was vouched for.
    let receipt_fingerprint = fingerprint(&delivered);

    if body.tamper {
        if let Some(stamp) = delivered.first_mut() {
            stamp.value = match &stamp.value {
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
        &company::Checker { company: &requester, request_id: &id, now: now() },
        state.registry.describe(),
        state.registry.explorer_url(),
    );

    let status = {
        let mut requests = state.requests.lock().unwrap();
        let request = requests.get_mut(&id).ok_or_else(|| error(StatusCode::NOT_FOUND, "No such request"))?;
        request.status = if verdict.ok { "verified" } else { "rejected" }.to_string();
        request.verdict = Some(verdict);
        request.delivered = delivered;
        request.expires_at = Some(expires_at);
        request.receipt = Some(Receipt {
            fingerprint: receipt_fingerprint.clone(),
            status: "pending".to_string(),
            detail: None,
        });
        request.status.clone()
    };

    // Step 7: write the receipt to Stellar in the background (it takes a few
    // seconds), so the person and the company are not kept waiting.
    let background = state.clone();
    let receipt_id = id.clone();
    tokio::spawn(async move {
        let outcome = background.registry.record_receipt(&receipt_fingerprint).await;
        let mut requests = background.requests.lock().unwrap();
        if let Some(receipt) = requests.get_mut(&receipt_id).and_then(|r| r.receipt.as_mut()) {
            match outcome {
                Ok(()) => receipt.status = "recorded".to_string(),
                Err(why) => {
                    receipt.status = "not recorded".to_string();
                    receipt.detail = Some(why);
                }
            }
        }
    });

    // Finally: once the company has had time to collect the facts, OnePass
    // wipes its own copy. Only the receipt and the names of the facts remain.
    let background = state.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(RETENTION_SECS)).await;
        let mut requests = background.requests.lock().unwrap();
        if let Some(request) = requests.get_mut(&id) {
            request.delivered.clear();
            if let Some(verdict) = request.verdict.as_mut() {
                for check in verdict.checks.iter_mut() {
                    check.value = None;
                }
            }
            request.purged = true;
        }
    });

    Ok(Json(json!({ "status": status })))
}

#[derive(Deserialize)]
struct Replay {
    /// Stamps a company received earlier, now being shown to someone else.
    stamps: Vec<Stamp>,
    /// The company they are being shown to.
    as_company: String,
}

/// POST /api/replay — DEMO: what happens if a company passes its stamps on?
/// A second company runs the normal checks on stamps that were issued to the
/// first. The signatures are genuine, but the stamps are not made out to it.
async fn replay(State(state): State<Shared>, Json(body): Json<Replay>) -> Result<Json<company::Verdict>, ApiError> {
    let trusted = state.registry.trusted().await.map_err(|e| error(StatusCode::BAD_GATEWAY, e))?;
    let wants: Vec<String> = body.stamps.iter().map(|s| s.field.clone()).collect();
    Ok(Json(company::verify(
        &wants,
        &body.stamps,
        &trusted,
        // The second company's own request would have a different id.
        &company::Checker { company: &body.as_company, request_id: "a-different-request", now: now() },
        state.registry.describe(),
        state.registry.explorer_url(),
    )))
}

/// POST /api/requests/{id}/deny — the person says no. Nothing is fetched or shared.
async fn deny(State(state): State<Shared>, Path(id): Path<String>) -> Result<Json<Value>, ApiError> {
    let mut requests = state.requests.lock().unwrap();
    let request = requests.get_mut(&id).ok_or_else(|| error(StatusCode::NOT_FOUND, "No such request"))?;
    if request.status == "pending" {
        request.status = "denied".to_string();
    }
    Ok(Json(json!({ "status": request.status })))
}

// ───────────────────────────────── Web pages ─────────────────────────────────
// The pages are plain HTML files in server/static, baked into the program.

async fn landing_page() -> Html<&'static str> {
    Html(include_str!("../static/landing.html"))
}
async fn user_page() -> Html<&'static str> {
    Html(include_str!("../static/app.html"))
}
async fn requester_page() -> Html<&'static str> {
    Html(include_str!("../static/request.html"))
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
        .route("/", get(landing_page))
        .route("/app", get(user_page))
        .route("/request", get(requester_page))
        .route("/style.css", get(style))
        .route("/app.js", get(script))
        .route("/api/users", get(list_users))
        .route("/api/catalog", get(catalog))
        .route("/api/sources", get(list_sources))
        .route("/api/connect", post(connect))
        .route("/api/disconnect", post(disconnect))
        .route("/api/requests", post(create_request))
        .route("/api/requests/{id}", get(get_request))
        .route("/api/requests/{id}/approve", post(approve))
        .route("/api/requests/{id}/deny", post(deny))
        .route("/api/notifications", get(notifications))
        .route("/api/replay", post(replay))
        .route("/api/demo/company", get(demo_company).post(set_demo_company))
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
