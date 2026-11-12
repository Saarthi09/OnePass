//! OnePass stand-in institutions.
//!
//! OnePass cannot connect to real banks or universities for this project, so
//! this program plays their part: three invented institutions, each with its
//! own records and its own signing key, all served from one small web server.
//!
//! What each institution can do:
//!   GET  /sources                    list the institutions and their public keys
//!   POST /sources/{id}/login         pretend "sign in" -> returns an access token
//!   GET  /sources/{id}/facts?fields= hand out signed facts about the signed-in person

mod data;
mod signing;

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use rand::RngCore;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

/// Everything the server remembers while it runs.
struct AppState {
    institutions: Vec<data::Institution>,
    /// access token -> (institution id, username). Filled in when someone logs in.
    tokens: Mutex<HashMap<String, (String, String)>>,
}

type Shared = Arc<AppState>;
/// An error reply: a status code (like 404) plus a JSON message.
type ApiError = (StatusCode, Json<Value>);

fn error(status: StatusCode, message: &str) -> ApiError {
    (status, Json(json!({ "error": message })))
}

fn find<'a>(state: &'a AppState, id: &str) -> Result<&'a data::Institution, ApiError> {
    state
        .institutions
        .iter()
        .find(|i| i.id == id)
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "No such institution"))
}

/// GET /sources — who the institutions are and which facts each can provide.
/// The public keys listed here are what the Stellar registry will hold.
async fn list_sources(State(state): State<Shared>) -> Json<Value> {
    let list: Vec<Value> = state
        .institutions
        .iter()
        .map(|i| {
            // Collect every field name this institution has for anyone.
            let mut fields: Vec<&String> = i.records.values().flat_map(|r| r.fields.keys()).collect();
            fields.sort();
            fields.dedup();
            json!({
                "id": i.id,
                "name": i.name,
                "kind": i.kind,
                "public_key": signing::stellar_address(&signing::demo_key(&i.id).verifying_key()),
                "fields": fields,
            })
        })
        .collect();
    Json(json!({ "demo": true, "sources": list }))
}

#[derive(Deserialize)]
struct Login {
    username: String,
    password: String,
}

/// POST /sources/{id}/login — the pretend "Sign in with Northfield University".
/// On success the institution returns a random token. Holding that token is
/// what lets OnePass fetch this person's facts later, without knowing the password.
async fn login(
    State(state): State<Shared>,
    Path(id): Path<String>,
    Json(body): Json<Login>,
) -> Result<Json<Value>, ApiError> {
    let institution = find(&state, &id)?;
    let known = institution
        .records
        .get(&body.username)
        .is_some_and(|r| r.password == body.password);
    if !known {
        return Err(error(StatusCode::UNAUTHORIZED, "Wrong username or password"));
    }

    let mut bytes = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut bytes);
    let token: String = bytes.iter().map(|b| format!("{b:02x}")).collect();

    state
        .tokens
        .lock()
        .unwrap()
        .insert(token.clone(), (id, body.username));
    Ok(Json(json!({ "token": token })))
}

#[derive(Deserialize)]
struct FactsQuery {
    /// Comma-separated field names, e.g. "is_student,program"
    fields: String,
}

/// GET /sources/{id}/facts?fields=a,b — hand out only the facts asked for,
/// each one signed separately so it can be shared on its own.
async fn facts(
    State(state): State<Shared>,
    Path(id): Path<String>,
    Query(query): Query<FactsQuery>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let institution = find(&state, &id)?;

    // The token arrives in a header that looks like: "Authorization: Bearer abc123"
    let token = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or_else(|| error(StatusCode::UNAUTHORIZED, "Missing access token"))?;

    // Which person does this token belong to, and is it for this institution?
    let username = match state.tokens.lock().unwrap().get(token) {
        Some((token_institution, username)) if *token_institution == id => username.clone(),
        _ => return Err(error(StatusCode::UNAUTHORIZED, "Invalid access token")),
    };
    let record = &institution.records[&username];

    let key = signing::demo_key(&institution.id);
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

    let mut signed = Vec::new();
    for field in query.fields.split(',').map(str::trim).filter(|f| !f.is_empty()) {
        let value = record
            .fields
            .get(field)
            .ok_or_else(|| error(StatusCode::NOT_FOUND, &format!("{} has no field '{field}'", institution.name)))?;
        signed.push(signing::sign_fact(&key, field, value, now));
    }

    Ok(Json(json!({ "source": institution.name, "facts": signed })))
}

#[tokio::main]
async fn main() {
    let state: Shared = Arc::new(AppState {
        institutions: data::load(),
        tokens: Mutex::new(HashMap::new()),
    });

    println!("OnePass stand-in institutions (demo data only):");
    for i in &state.institutions {
        let key = signing::stellar_address(&signing::demo_key(&i.id).verifying_key());
        println!("  {:<24} {key}", i.name);
    }

    let app = Router::new()
        .route("/sources", get(list_sources))
        .route("/sources/{id}/login", post(login))
        .route("/sources/{id}/facts", get(facts))
        .with_state(state);

    let address = "127.0.0.1:4001";
    let listener = tokio::net::TcpListener::bind(address).await.expect("port 4001 is already in use");
    println!("Listening on http://{address}");
    axum::serve(listener, app).await.unwrap();
}
