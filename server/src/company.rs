//! The company's side of OnePass.
//!
//! A company never has to trust OnePass. It checks every fact itself:
//!   1. Is the signature genuine and the fact unaltered?
//!   2. Is the institution that signed it on the trusted registry?
//!   3. Did I receive everything I asked for, from the kind of institution I asked for?

use crate::registry::TrustedSource;
use onepass_core::{verify_fact, SignedFact};
use serde::Serialize;
use serde_json::Value;

/// One thing a company wants to know, and what kind of institution must vouch for it.
#[derive(Clone, Serialize)]
pub struct Want {
    pub field: &'static str,
    pub kind: &'static str,
}

/// A demo company and what it asks for.
#[derive(Clone, Serialize)]
pub struct Company {
    pub id: &'static str,
    pub name: &'static str,
    pub tagline: &'static str,
    pub action: &'static str,
    pub success: &'static str,
    pub wants: Vec<Want>,
}

/// The two invented companies used in the demo.
pub fn companies() -> Vec<Company> {
    vec![
        Company {
            id: "stride-sportswear",
            name: "Stride Sportswear",
            tagline: "20% off for students. No student card photos, no forms.",
            action: "Get student discount",
            success: "Student discount applied",
            wants: vec![Want { field: "is_student", kind: "university" }],
        },
        Company {
            id: "brightline-mobile",
            name: "Brightline Mobile",
            tagline: "Open a phone plan in one tap, with details verified by your bank.",
            action: "Sign up",
            success: "Account opened",
            wants: vec![
                Want { field: "full_name", kind: "bank" },
                Want { field: "address", kind: "bank" },
                Want { field: "date_of_birth", kind: "bank" },
            ],
        },
    ]
}

/// The outcome of checking one fact (or noticing it is missing).
#[derive(Clone, Serialize)]
pub struct FactCheck {
    pub field: String,
    pub value: Option<Value>,
    pub source_name: Option<String>,
    pub received: bool,
    pub signature_ok: bool,
    pub source_trusted: bool,
    pub right_kind: bool,
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

/// Run the company's checks on the facts the user shared.
pub fn verify(
    wants: &[Want],
    shared: &[SignedFact],
    trusted: &[TrustedSource],
    registry: String,
    registry_url: Option<String>,
) -> Verdict {
    let checks: Vec<FactCheck> = wants
        .iter()
        .map(|want| {
            let Some(fact) = shared.iter().find(|f| f.field == want.field) else {
                // The user chose not to share this one.
                return FactCheck {
                    field: want.field.to_string(),
                    value: None,
                    source_name: None,
                    received: false,
                    signature_ok: false,
                    source_trusted: false,
                    right_kind: false,
                    ok: false,
                };
            };
            let signature_ok = verify_fact(fact);
            let source = trusted.iter().find(|t| t.public_key == fact.source_key);
            let right_kind = source.is_some_and(|s| s.kind == want.kind);
            FactCheck {
                field: want.field.to_string(),
                value: Some(fact.value.clone()),
                source_name: source.map(|s| s.name.clone()),
                received: true,
                signature_ok,
                source_trusted: source.is_some(),
                right_kind,
                ok: signature_ok && right_kind,
            }
        })
        .collect();

    Verdict { ok: checks.iter().all(|c| c.ok), registry, registry_url, checks }
}
