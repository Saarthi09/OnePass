//! The company's side of OnePass.
//!
//! A company (a recruiter, a shop, a phone carrier) uses the requester page to
//! ask OnePass for facts about a person. It never learns which institution a fact came from. What
//! it receives is each fact stamped "verified by OnePass": signed with
//! OnePass's own key.
//!
//! The company's checks:
//!   1. Is OnePass's signature on each fact genuine and the fact unaltered?
//!   2. Is the key that signed it really OnePass's? It looks the key up on the
//!      Stellar registry rather than taking anyone's word for it.
//!   3. Did it receive everything it asked for?

use crate::registry::TrustedSource;
use onepass_core::{verify_fact, SignedFact};
use serde::Serialize;
use serde_json::Value;

/// The registry "kind" that marks OnePass's own verifier key.
pub const VERIFIER_KIND: &str = "verifier";

/// Every fact a company may ask for.
pub const FIELDS: &[&str] = &[
    "full_name",
    "date_of_birth",
    "over_18",
    "address",
    "is_student",
    "program",
    "enrolment_year",
    "student_id",
    "is_tenant",
    "tenancy_start",
    "customer_since",
];

/// A ready-made bundle of facts for a common purpose, offered as a shortcut
/// on the requester page.
#[derive(Clone, Serialize)]
pub struct Preset {
    pub name: &'static str,
    pub reason: &'static str,
    pub fields: Vec<&'static str>,
}

pub fn presets() -> Vec<Preset> {
    vec![
        Preset {
            name: "Student discount",
            reason: "To confirm you qualify for our student discount",
            fields: vec!["is_student"],
        },
        Preset {
            name: "Account sign-up",
            reason: "To open your account without a form",
            fields: vec!["full_name", "address", "date_of_birth"],
        },
        Preset {
            name: "Background check",
            reason: "Background check for your job application",
            fields: vec!["full_name", "date_of_birth", "address", "is_student"],
        },
    ]
}

/// The outcome of checking one fact (or noticing it is missing).
#[derive(Clone, Serialize)]
pub struct FactCheck {
    pub field: String,
    pub value: Option<Value>,
    pub received: bool,
    /// OnePass's signature matches and the fact was not altered.
    pub signature_ok: bool,
    /// The signing key is listed on the registry as OnePass's verifier key.
    pub signer_is_onepass: bool,
    pub ok: bool,
}

#[derive(Clone, Serialize)]
pub struct Verdict {
    pub ok: bool,
    /// Which registry the company consulted, and a link to inspect it.
    pub registry: String,
    pub registry_url: Option<String>,
    pub checks: Vec<FactCheck>,
}

/// Run the company's checks on what OnePass delivered.
pub fn verify(
    wants: &[String],
    delivered: &[SignedFact],
    trusted: &[TrustedSource],
    registry: String,
    registry_url: Option<String>,
) -> Verdict {
    let checks: Vec<FactCheck> = wants
        .iter()
        .map(|want| {
            let Some(fact) = delivered.iter().find(|f| f.field == *want) else {
                // The person chose not to share this one.
                return FactCheck {
                    field: want.to_string(),
                    value: None,
                    received: false,
                    signature_ok: false,
                    signer_is_onepass: false,
                    ok: false,
                };
            };
            let signature_ok = verify_fact(fact);
            let signer_is_onepass = trusted
                .iter()
                .any(|t| t.public_key == fact.source_key && t.kind == VERIFIER_KIND);
            FactCheck {
                field: want.to_string(),
                value: Some(fact.value.clone()),
                received: true,
                signature_ok,
                signer_is_onepass,
                ok: signature_ok && signer_is_onepass,
            }
        })
        .collect();

    Verdict { ok: checks.iter().all(|c| c.ok), registry, registry_url, checks }
}
