//! The OnePass server.
//!
//! What it does:
//!   * lets the user connect their institutions (it keeps an access token, never the data);
//!   * when a company asks for facts, fetches just those facts from the right institution;
//!   * shows the user exactly what would be shared, and waits for approval;
//!   * passes the approved, signed facts to the company, which checks them itself.
//!
//! It also serves the web pages, including two invented companies for the demo.
//! Everything is kept in memory, so restarting the server resets the demo.

mod company;
mod registry;

use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use onepass_core::SignedFact;
use rand::RngCore;
use registry::TrustedSource;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

/// Where the stand-in institutions are running.
const SOURCES_URL: &str = "http://127.0.0.1:4001";

/// One line of a request: a fact the company wants, and what OnePass found for it.
#[derive(Clone, Serialize)]
struct Item {
    field: String,
    /// Kind of institution the company wants it from, e.g. "bank".
    kind: String,
    /// Which connected institution has it (None = the user has not connected one).
    source_name: Option<String>,
    /// The signed fact fetched from that institution.
    fact: Option<SignedFact>,
}

/// A company's request, from creation to the final verdict.
#[derive(Clone, Serialize)]
struct Request {
    id: String,
    company: company::Company,
    /// "pending", "verified", "rejected" or "denied"
    status: String,
    items: Vec<Item>,
    verdict: Option<company::Verdict>,
}

/// Everything the server remembers while it runs.
struct AppState {
    client: reqwest::Client,
    /// institution id -> access token. This is ALL OnePass stores about the user's accounts.
    connections: Mutex<HashMap<String, String>>,
    requests: Mutex<HashMap<String, Request>>,
}

type Shared = Arc<AppState>;
type ApiError = (StatusCode, Json<Value>);

fn error(status: StatusCode, message: impl Into<String>) -> ApiError {
    (status, Json(json!({ "error": message.into() })))
}

async fn trusted_sources(state: &AppState) -> Result<Vec<TrustedSource>, ApiError> {
    registry::load(&state.client, SOURCES_URL)
        .await
        .map_err(|e| error(StatusCode::BAD_GATEWAY, e))
}

fn random_id() -> String {
    let mut bytes = [0u8; 8];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ───────────────────────── Connecting institutions ─────────────────────────

/// GET /api/sources — every institution, and whether the user has connected it.
async fn list_sources(State(state): State<Shared>) -> Result<Json<Value>, ApiError> {
    let sources = trusted_sources(&state).await?;
    let connections = state.connections.lock().unwrap();
    let list: Vec<Value> = sources
        .iter()
        .map(|s| {
            json!({
                "id": s.id, "name": s.name, "kind": s.kind, "public_key": s.public_key,
                "fields": s.fields, "connected": connections.contains_key(&s.id),
            })
        })
        .collect();
    Ok(Json(json!({ "sources": list })))
}

#[derive(Deserialize)]
struct Connect {
    source_id: String,
    username: String,
    password: String,
}

/// POST /api/connect — sign in to an institution on the user's behalf.
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

// ───────────────────────────── Company requests ─────────────────────────────

/// GET /api/companies — the demo companies and what each asks for.
async fn list_companies() -> Json<Value> {
    Json(json!({ "companies": company::companies() }))
}

#[derive(Deserialize)]
struct NewRequest {
    company_id: String,
}

/// POST /api/requests — a company asks for facts about the user.
/// OnePass works out which connected institution has each fact and fetches it,
/// but shares nothing yet: the request waits for the user's approval.
async fn create_request(State(state): State<Shared>, Json(body): Json<NewRequest>) -> Result<Json<Value>, ApiError> {
    let company = company::companies()
        .into_iter()
        .find(|c| c.id == body.company_id)
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "No such company"))?;
    let sources = trusted_sources(&state).await?;
    // Copy the tokens out so the lock is released before we make network calls.
    let connections = state.connections.lock().unwrap().clone();

    let mut items = Vec::new();
    for want in &company.wants {
        // Find a connected institution of the right kind that has this fact.
        let source = sources.iter().find(|s| {
            s.kind == want.kind && connections.contains_key(&s.id) && s.fields.iter().any(|f| f == want.field)
        });
        let mut item = Item {
            field: want.field.to_string(),
            kind: want.kind.to_string(),
            source_name: source.map(|s| s.name.clone()),
            fact: None,
        };
        if let Some(source) = source {
            // Ask the institution for this one fact, using the stored token.
            let reply: Value = state
                .client
                .get(format!("{SOURCES_URL}/sources/{}/facts?fields={}", source.id, want.field))
                .bearer_auth(&connections[&source.id])
                .send()
                .await
                .map_err(|_| error(StatusCode::BAD_GATEWAY, "Could not reach the institution"))?
                .json()
                .await
                .map_err(|e| error(StatusCode::BAD_GATEWAY, e.to_string()))?;
            item.fact = serde_json::from_value(reply["facts"][0].clone()).ok();
            if item.fact.is_none() {
                // Token no longer accepted (e.g. the institutions were restarted).
                item.source_name = None;
            }
        }
        items.push(item);
    }

    let request = Request { id: random_id(), company, status: "pending".into(), items, verdict: None };
    let id = request.id.clone();
    state.requests.lock().unwrap().insert(id.clone(), request);
    Ok(Json(json!({ "id": id })))
}

/// GET /api/requests/{id} — the current state of a request.
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

#[derive(Deserialize)]
struct Approval {
    /// The facts the user ticked.
    fields: Vec<String>,
    /// Demo switch: alter a fact on the way to the company, to show it gets caught.
    #[serde(default)]
    tamper: bool,
}

/// POST /api/requests/{id}/approve — the user agrees to share the ticked facts.
/// Only those facts are passed to the company, which then runs its own checks.
async fn approve(
    State(state): State<Shared>,
    Path(id): Path<String>,
    Json(body): Json<Approval>,
) -> Result<Json<Request>, ApiError> {
    let trusted = trusted_sources(&state).await?;
    let mut requests = state.requests.lock().unwrap();
    let request = requests.get_mut(&id).ok_or_else(|| error(StatusCode::NOT_FOUND, "No such request"))?;
    if request.status != "pending" {
        return Err(error(StatusCode::CONFLICT, "This request was already answered"));
    }

    let mut shared: Vec<SignedFact> = request
        .items
        .iter()
        .filter(|i| body.fields.contains(&i.field))
        .filter_map(|i| i.fact.clone())
        .collect();

    if body.tamper {
        if let Some(fact) = shared.first_mut() {
            fact.value = match &fact.value {
                Value::Bool(b) => json!(!b),
                _ => json!("Someone Else"),
            };
        }
    }

    let verdict = company::verify(&request.company.wants, &shared, &trusted, registry::REGISTRY_NAME);
    request.status = if verdict.ok { "verified" } else { "rejected" }.to_string();
    request.verdict = Some(verdict);
    Ok(Json(request.clone()))
}

/// POST /api/requests/{id}/deny — the user refuses. Nothing is shared.
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
async fn approve_page() -> Html<&'static str> {
    Html(include_str!("../static/approve.html"))
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
        connections: Mutex::new(HashMap::new()),
        requests: Mutex::new(HashMap::new()),
    });

    let app = Router::new()
        .route("/", get(home_page))
        .route("/company/{id}", get(company_page))
        .route("/approve/{id}", get(approve_page))
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
        .with_state(state);

    let address = "127.0.0.1:4000";
    let listener = tokio::net::TcpListener::bind(address).await.expect("port 4000 is already in use");
    println!("OnePass is running. Open http://{address} in your browser.");
    println!("(The stand-in institutions must also be running: cargo run -p onepass-sources)");
    axum::serve(listener, app).await.unwrap();
}
