//! The registry of trusted institutions.
//!
//! This answers one question: "which signing keys belong to genuine institutions?"
//!
//! STELLAR MODE (the real thing): the list lives in a smart contract on the
//! Stellar testnet (see contracts/registry). This file asks the contract for
//! the list by running Stellar's command-line tool:
//!
//!     stellar contract invoke --id <contract> ... -- list
//!
//! Reading is free and changes nothing on the blockchain. Stellar mode is
//! switched on by the file `stellar-registry.txt`, which `scripts/setup-stellar.sh`
//! creates when it deploys the contract.
//!
//! LOCAL MODE (fallback): if that file is missing, a copy of the list written
//! in this file is used instead, and the pages say so.

use serde::{Deserialize, Serialize};
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

/// File holding the deployed contract's id. Created by scripts/setup-stellar.sh.
const CONTRACT_FILE: &str = "stellar-registry.txt";
/// The Stellar CLI identity used to ask the question (reading needs no special rights).
const STELLAR_IDENTITY: &str = "onepass-admin";
/// How long an answer is reused before asking the blockchain again.
const CACHE_FOR: Duration = Duration::from_secs(20);

/// One institution on the registry.
#[derive(Clone, Serialize, Deserialize)]
pub struct TrustedSource {
    pub public_key: String,
    pub name: String,
    pub kind: String,
}

pub struct Registry {
    /// Some(id) = Stellar mode, None = local mode.
    contract_id: Option<String>,
    cache: Mutex<Option<(Instant, Vec<TrustedSource>)>>,
}

impl Registry {
    /// Decide which mode to run in, by looking for the contract-id file.
    pub fn detect() -> Registry {
        let contract_id = std::fs::read_to_string(CONTRACT_FILE)
            .ok()
            .map(|text| text.trim().to_string())
            .filter(|id| !id.is_empty());
        Registry { contract_id, cache: Mutex::new(None) }
    }

    /// A human-readable description, shown on the result page.
    pub fn describe(&self) -> String {
        match &self.contract_id {
            Some(id) => format!("Stellar testnet smart contract {id}"),
            None => "local copy (Stellar not set up yet: run scripts/setup-stellar.sh)".to_string(),
        }
    }

    /// A link where anyone can inspect the contract and its history.
    pub fn explorer_url(&self) -> Option<String> {
        self.contract_id
            .as_ref()
            .map(|id| format!("https://stellar.expert/explorer/testnet/contract/{id}"))
    }

    /// The current list of trusted institutions.
    pub async fn trusted(&self) -> Result<Vec<TrustedSource>, String> {
        let Some(contract_id) = &self.contract_id else {
            return Ok(local_list());
        };

        // Reuse a recent answer if we have one.
        if let Some((when, list)) = &*self.cache.lock().unwrap() {
            if when.elapsed() < CACHE_FOR {
                return Ok(list.clone());
            }
        }

        let output = tokio::process::Command::new("stellar")
            .args(["contract", "invoke", "--id", contract_id])
            .args(["--source-account", STELLAR_IDENTITY, "--network", "testnet"])
            .args(["--", "list"])
            .output()
            .await
            .map_err(|e| format!("Could not run the Stellar CLI (is it installed?): {e}"))?;

        if !output.status.success() {
            return Err(format!(
                "The Stellar registry lookup failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }

        // The contract's answer is printed as JSON, e.g.
        // [{"kind":"bank","name":"Maple Trust Bank","public_key":"GA4K..."}]
        let text = String::from_utf8_lossy(&output.stdout);
        let list: Vec<TrustedSource> = serde_json::from_str(text.trim())
            .map_err(|e| format!("Could not read the registry's answer ({e}): {}", text.trim()))?;

        *self.cache.lock().unwrap() = Some((Instant::now(), list.clone()));
        Ok(list)
    }
}

/// Fallback list for local mode: the same three institutions the setup script
/// puts on Stellar. "Diploma Express" is deliberately absent.
fn local_list() -> Vec<TrustedSource> {
    [
        ("GCKSFO63Z4OVXXULTTFQJNAKBSYTEI5VIKZISTUCITO77OCUO7VPZA4W", "Northfield University", "university"),
        ("GA4K6NUZFXL3USVVOGVU2ZQD2YAOLQNDS2KITPVYWZQ7V2I3XXD76Z7R", "Maple Trust Bank", "bank"),
        ("GAGUIDWAQY6FV3AF5WMGHW2TYQOQH6PWYMA33BM5MASB7HQ4UFKFNFWR", "CityHome Housing", "housing"),
    ]
    .into_iter()
    .map(|(public_key, name, kind)| TrustedSource {
        public_key: public_key.to_string(),
        name: name.to_string(),
        kind: kind.to_string(),
    })
    .collect()
}
