//! The company's side of OnePass.
//!
//! A company (a recruiter, a shop, a phone carrier) asks OnePass for facts
//! about a person. It never learns which institution a fact came from. What
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

/// A demo company and what it asks for.
#[derive(Clone, Serialize)]
pub struct Company {
    pub id: &'static str,
    pub name: &'static str,
    pub tagline: &'static str,
    pub action: &'static str,
    pub success: &'static str,
    /// The facts it wants, by name. It does not get to say where they come from.
    pub wants: Vec<&'static str>,
}

/// The invented companies used in the demo.
pub fn companies() -> Vec<Company> {
    vec![
        Company {
            id: "stride-sportswear",
            name: "Stride Sportswear",
            tagline: "20% off for students. No student card photos, no forms.",
            action: "Get student discount",
            success: "Student discount applied",
            wants: vec!["is_student"],
        },
        Company {
            id: "brightline-mobile",
            name: "Brightline Mobile",
            tagline: "Open a phone plan in one tap. No forms, no document uploads.",
            action: "Sign up",
            success: "Account opened",
            wants: vec!["full_name", "address", "date_of_birth"],
        },
        Company {
            id: "harbour-talent",
            name: "Harbour Talent",
            tagline: "Recruiter background check: identity and education, without the paperwork.",
            action: "Send background check",
            success: "Background check complete",
            wants: vec!["full_name", "date_of_birth", "address", "is_student", "program"],
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
    wants: &[&str],
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
