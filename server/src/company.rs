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
//!   3. Was the stamp issued to this company, for this request, and is it still
//!      in date? (This stops stamps being passed on or reused.)
//!   4. Did it receive everything it asked for?

use crate::registry::TrustedSource;
use onepass_core::{demo_key, sign_text, stellar_address, verify_stamp, verify_text, SigningKey, Stamp};
use serde::Serialize;
use serde_json::Value;

/// The registry "kind" that marks OnePass's own verifier key.
pub const VERIFIER_KIND: &str = "verifier";

/// The registry "kind" that marks a company allowed to make requests under its name.
pub const COMPANY_KIND: &str = "company";

/// How old a signed request may be before OnePass ignores its signature (5 minutes).
/// This stops someone copying an old signed request and sending it again later.
const REQUEST_FRESH_SECS: u64 = 300;

// ─────────────────── Proving which company is asking ───────────────────
//
// A company that has registered with OnePass holds a signing key, and its
// public half is listed on the Stellar registry next to the company's name.
// The company signs every request. OnePass checks the signature and looks the
// key up on Stellar. Typing a company's name is not enough: without that
// company's secret key, the signature cannot be produced.

/// The proof a company attaches to a request.
pub struct RequestProof {
    /// The company's public key, in Stellar "G..." format.
    pub key: String,
    /// When the request was signed. Seconds since 1 Jan 1970.
    pub signed_at: u64,
    /// The company's signature over the whole request, in base64.
    pub signature: String,
}

/// The exact text a company signs. It covers everything in the request, so
/// none of it can be changed afterwards.
fn request_message(requester: &str, user: &str, wants: &[String], reason: &str, signed_at: u64) -> String {
    format!("onepass-request-v1\n{requester}\n{user}\n{}\n{reason}\n{signed_at}", wants.join(","))
}

/// The signing key a registered demo company holds.
///
/// DEMO ONLY: in real use each company keeps its key on its own servers and
/// signs there. Here the OnePass server plays the company's part, and only
/// Harbour Talent has registered. Any other name has no key and sends its
/// request unsigned.
fn registered_key(requester: &str) -> Option<SigningKey> {
    (requester == "Harbour Talent").then(|| demo_key("company:harbour-talent"))
}

/// The company's side: sign a request, if this company has a key.
/// `impostor` (demo) signs with a made-up key instead, as a scammer using the
/// company's name would have to.
pub fn sign_request(
    requester: &str,
    user: &str,
    wants: &[String],
    reason: &str,
    signed_at: u64,
    impostor: bool,
) -> Option<RequestProof> {
    let key = if impostor { demo_key("company:impostor") } else { registered_key(requester)? };
    Some(RequestProof {
        key: stellar_address(&key.verifying_key()),
        signed_at,
        signature: sign_text(&key, &request_message(requester, user, wants, reason, signed_at)),
    })
}

/// OnePass's side: who is really asking?
///   "verified"  = signed with the key registered to this company on Stellar
///   "wrong_key" = signed, but not with a key registered to this company
///   "unsigned"  = no proof at all
pub fn check_requester(
    requester: &str,
    user: &str,
    wants: &[String],
    reason: &str,
    proof: Option<&RequestProof>,
    trusted: &[TrustedSource],
    now: u64,
) -> &'static str {
    let Some(proof) = proof else {
        return "unsigned";
    };
    let genuine = verify_text(&proof.key, &request_message(requester, user, wants, reason, proof.signed_at), &proof.signature);
    let fresh = now.abs_diff(proof.signed_at) <= REQUEST_FRESH_SECS;
    let registered = trusted
        .iter()
        .any(|t| t.public_key == proof.key && t.kind == COMPANY_KIND && t.name == requester);
    if genuine && fresh && registered {
        "verified"
    } else {
        "wrong_key"
    }
}

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

/// The outcome of checking one stamped fact (or noticing it is missing).
#[derive(Clone, Serialize)]
pub struct FactCheck {
    pub field: String,
    pub value: Option<Value>,
    pub received: bool,
    /// OnePass's signature matches and nothing in the stamp was altered.
    pub signature_ok: bool,
    /// The signing key is listed on the registry as OnePass's verifier key.
    pub signer_is_onepass: bool,
    /// The stamp was issued to this company, for this request.
    pub issued_to_me: bool,
    /// The stamp has not expired.
    pub in_date: bool,
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

/// Who is checking, and when. A company knows its own name, which request it
/// made, and what time it is; it compares the stamps against those.
pub struct Checker<'a> {
    pub company: &'a str,
    pub request_id: &'a str,
    pub now: u64,
}

/// Run the company's checks on what OnePass delivered.
pub fn verify(
    wants: &[String],
    delivered: &[Stamp],
    trusted: &[TrustedSource],
    checker: &Checker,
    registry: String,
    registry_url: Option<String>,
) -> Verdict {
    let checks: Vec<FactCheck> = wants
        .iter()
        .map(|want| {
            let Some(stamp) = delivered.iter().find(|s| &s.field == want) else {
                // The person chose not to share this one.
                return FactCheck {
                    field: want.to_string(),
                    value: None,
                    received: false,
                    signature_ok: false,
                    signer_is_onepass: false,
                    issued_to_me: false,
                    in_date: false,
                    ok: false,
                };
            };
            let signature_ok = verify_stamp(stamp);
            let signer_is_onepass = trusted
                .iter()
                .any(|t| t.public_key == stamp.signer_key && t.kind == VERIFIER_KIND);
            let issued_to_me = stamp.audience == checker.company && stamp.request_id == checker.request_id;
            let in_date = checker.now < stamp.expires_at;
            FactCheck {
                field: want.to_string(),
                value: Some(stamp.value.clone()),
                received: true,
                signature_ok,
                signer_is_onepass,
                issued_to_me,
                in_date,
                ok: signature_ok && signer_is_onepass && issued_to_me && in_date,
            }
        })
        .collect();

    Verdict { ok: checks.iter().all(|c| c.ok), registry, registry_url, checks }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> Vec<TrustedSource> {
        let key = registered_key("Harbour Talent").unwrap();
        vec![TrustedSource {
            public_key: stellar_address(&key.verifying_key()),
            name: "Harbour Talent".to_string(),
            kind: COMPANY_KIND.to_string(),
        }]
    }

    fn check(requester: &str, impostor: bool, trusted: &[TrustedSource], now: u64) -> &'static str {
        let wants = vec!["full_name".to_string()];
        let proof = sign_request(requester, "alex", &wants, "Job application", 1000, impostor);
        check_requester(requester, "alex", &wants, "Job application", proof.as_ref(), trusted, now)
    }

    #[test]
    fn registered_company_is_verified() {
        assert_eq!(check("Harbour Talent", false, &registry(), 1010), "verified");
    }

    #[test]
    fn impostor_using_the_name_is_caught() {
        assert_eq!(check("Harbour Talent", true, &registry(), 1010), "wrong_key");
    }

    #[test]
    fn company_without_a_key_is_unsigned() {
        assert_eq!(check("Corner Shop", false, &registry(), 1010), "unsigned");
    }

    #[test]
    fn company_removed_from_the_registry_is_not_verified() {
        assert_eq!(check("Harbour Talent", false, &[], 1010), "wrong_key");
    }

    #[test]
    fn old_signed_request_is_not_accepted() {
        assert_eq!(check("Harbour Talent", false, &registry(), 1000 + 3600), "wrong_key");
    }

    #[test]
    fn changing_a_signed_request_breaks_it() {
        let wants = vec!["full_name".to_string()];
        let proof = sign_request("Harbour Talent", "alex", &wants, "Job application", 1000, false);
        let more = vec!["full_name".to_string(), "address".to_string()];
        assert_eq!(
            check_requester("Harbour Talent", "alex", &more, "Job application", proof.as_ref(), &registry(), 1010),
            "wrong_key"
        );
    }
}
