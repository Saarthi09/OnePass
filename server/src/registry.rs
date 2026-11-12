//! The registry of trusted institutions.
//!
//! This answers one question: "which public keys belong to genuine institutions?"
//!
//! RIGHT NOW it is a placeholder that simply asks the stand-in institutions for
//! their keys. That is circular (an impostor could vouch for itself), so the
//! next step is to replace `load` with a lookup on the Stellar blockchain,
//! where the list is public and cannot be quietly edited. Nothing else in the
//! project needs to change when that happens.

use serde::{Deserialize, Serialize};

pub const REGISTRY_NAME: &str = "local demo list (Stellar registry coming next)";

/// One institution on the registry.
#[derive(Clone, Serialize, Deserialize)]
pub struct TrustedSource {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub public_key: String,
    /// Which facts this institution can provide.
    pub fields: Vec<String>,
}

#[derive(Deserialize)]
struct SourceList {
    sources: Vec<TrustedSource>,
}

/// Fetch the current list of trusted institutions.
pub async fn load(client: &reqwest::Client, sources_url: &str) -> Result<Vec<TrustedSource>, String> {
    let response = client
        .get(format!("{sources_url}/sources"))
        .send()
        .await
        .map_err(|_| "The stand-in institutions are not running. Start them with: cargo run -p onepass-sources".to_string())?;
    let list: SourceList = response.json().await.map_err(|e| e.to_string())?;
    Ok(list.sources)
}
