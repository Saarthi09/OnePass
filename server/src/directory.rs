//! The directory: which institutions exist, and which facts each can provide.
//!
//! OnePass uses this to know where to fetch a fact from. It comes from the
//! institutions themselves, so it is NOT a source of trust: anyone can run a
//! server and advertise itself here. Trust comes only from the registry
//! (see registry.rs), which is why a company checks that instead.

use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct SourceInfo {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub public_key: String,
    pub fields: Vec<String>,
}

#[derive(Deserialize)]
struct SourceList {
    sources: Vec<SourceInfo>,
}

/// Ask the institutions' server who is available.
pub async fn load(client: &reqwest::Client, sources_url: &str) -> Result<Vec<SourceInfo>, String> {
    let response = client
        .get(format!("{sources_url}/sources"))
        .send()
        .await
        .map_err(|_| "The stand-in institutions are not running. Start them with: cargo run -p onepass-sources".to_string())?;
    let list: SourceList = response.json().await.map_err(|e| e.to_string())?;
    Ok(list.sources)
}
