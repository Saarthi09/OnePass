//! The registry of trusted institutions.
//!
//! This answers two questions:
//!   * "which signing keys belong to genuine institutions?" (OnePass asks this
//!     before vouching for a fact)
//!   * "which key is OnePass's own?" (companies ask this before accepting
//!     OnePass's stamp)
//!   * "which key belongs to this company?" (OnePass asks this before telling
//!     a person that a request really comes from the company it names)
//! It is also where OnePass records a receipt for every verification.
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

/// OnePass's own public key in the demo (the server checks this on startup).
pub const ONEPASS_KEY: &str = "GCYK7UIOCLXXHFAJSIPZ56TC5LPDLIFL25R6TK3W2ZSIGRYG4DZBF4ME";

/// One entry on the registry.
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
    /// The list used in local mode. It can change while the server runs.
    local: Mutex<Vec<TrustedSource>>,
}

impl Registry {
    /// Decide which mode to run in, by looking for the contract-id file.
    pub fn detect() -> Registry {
        let contract_id = std::fs::read_to_string(CONTRACT_FILE)
            .ok()
            .map(|text| text.trim().to_string())
            .filter(|id| !id.is_empty());
        Registry { contract_id, cache: Mutex::new(None), local: Mutex::new(local_list()) }
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
            return Ok(self.local.lock().unwrap().clone());
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

impl Registry {
    /// Add an entry to the registry (admin only). On Stellar this is a real
    /// transaction, signed by the registry's admin, so it takes a few seconds.
    pub async fn add(&self, public_key: &str, name: &str, kind: &str) -> Result<(), String> {
        if self.contract_id.is_none() {
            let mut list = self.local.lock().unwrap();
            list.retain(|entry| entry.public_key != public_key);
            list.push(TrustedSource {
                public_key: public_key.to_string(),
                name: name.to_string(),
                kind: kind.to_string(),
            });
            return Ok(());
        }
        self.change(&["add", "--public_key", public_key, "--name", name, "--kind", kind]).await
    }

    /// Take an entry off the registry (admin only).
    pub async fn remove(&self, public_key: &str) -> Result<(), String> {
        if self.contract_id.is_none() {
            self.local.lock().unwrap().retain(|entry| entry.public_key != public_key);
            return Ok(());
        }
        self.change(&["remove", "--public_key", public_key]).await
    }

    /// Send one change to the contract, then forget the remembered list so the
    /// next lookup sees the change straight away.
    async fn change(&self, call: &[&str]) -> Result<(), String> {
        let contract_id = self.contract_id.as_ref().expect("only called in Stellar mode");
        let output = tokio::process::Command::new("stellar")
            .args(["contract", "invoke", "--id", contract_id])
            .args(["--source-account", STELLAR_IDENTITY, "--network", "testnet"])
            .arg("--")
            .args(call)
            .output()
            .await
            .map_err(|e| format!("Could not run the Stellar CLI (is it installed?): {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "The Stellar registry change failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        *self.cache.lock().unwrap() = None;
        Ok(())
    }

    /// Record the fingerprint of one verification on Stellar.
    /// This is a real blockchain transaction, so it takes a few seconds.
    pub async fn record_receipt(&self, fingerprint: &str) -> Result<(), String> {
        let Some(contract_id) = &self.contract_id else {
            return Err("Stellar is not set up, so no receipt was recorded".to_string());
        };
        let output = tokio::process::Command::new("stellar")
            .args(["contract", "invoke", "--id", contract_id])
            .args(["--source-account", STELLAR_IDENTITY, "--network", "testnet"])
            .args(["--", "record", "--receipt", fingerprint])
            .output()
            .await
            .map_err(|e| format!("Could not run the Stellar CLI: {e}"))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
        }
    }
}

/// Fallback list for local mode: the same entries the setup script puts on
/// Stellar. "Diploma Express" is deliberately absent.
fn local_list() -> Vec<TrustedSource> {
    [
        ("GCKSFO63Z4OVXXULTTFQJNAKBSYTEI5VIKZISTUCITO77OCUO7VPZA4W", "Northfield University", "university"),
        ("GA4K6NUZFXL3USVVOGVU2ZQD2YAOLQNDS2KITPVYWZQ7V2I3XXD76Z7R", "Maple Trust Bank", "bank"),
        ("GAGUIDWAQY6FV3AF5WMGHW2TYQOQH6PWYMA33BM5MASB7HQ4UFKFNFWR", "CityHome Housing", "housing"),
        (ONEPASS_KEY, "OnePass", "verifier"),
        // A company that has registered, so its requests can be recognised.
        ("GDT4YT2YQDWX65RZEZFTPSCZWJG5ZNEJLAARBFQLRNY75WADENDLQQ6A", "Harbour Talent", "company"),
    ]
    .into_iter()
    .map(|(public_key, name, kind)| TrustedSource {
        public_key: public_key.to_string(),
        name: name.to_string(),
        kind: kind.to_string(),
    })
    .collect()
}
