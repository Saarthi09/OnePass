//! Digital signatures: how an institution proves a fact really came from it.
//!
//! Each institution has a key pair:
//!   * a SECRET key, which only the institution has, used to sign;
//!   * a PUBLIC key, which anyone may see, used to check a signature.
//!
//! If even one character of a signed fact is changed, the check fails.
//! The public key is written in Stellar's format (56 characters starting with
//! "G") so the same key can be listed in the Stellar registry later.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
pub use ed25519_dalek::SigningKey;
use ed25519_dalek::{Signature, Signer, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// One fact, signed by the institution it came from.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SignedFact {
    /// Public key of the institution, in Stellar "G..." format.
    pub source_key: String,
    /// What the fact is about, e.g. "is_student".
    pub field: String,
    /// The fact itself, e.g. true.
    pub value: Value,
    /// When it was signed (seconds since 1 Jan 1970).
    pub issued_at: u64,
    /// The institution's signature over everything above, in base64.
    pub signature: String,
}

/// Make an institution's secret key.
///
/// DEMO ONLY: the key is worked out from the institution's id, so it is the
/// same every time the program starts and anyone reading this code could
/// recreate it. A real institution would generate a random key and guard it.
pub fn demo_key(institution_id: &str) -> SigningKey {
    let seed: [u8; 32] = Sha256::digest(format!("onepass-demo-key:{institution_id}")).into();
    SigningKey::from_bytes(&seed)
}

/// Write a public key the way Stellar does ("G...").
pub fn stellar_address(key: &VerifyingKey) -> String {
    stellar_strkey::ed25519::PublicKey(key.to_bytes()).to_string()
}

/// The exact text that gets signed. Signer and checker must build it identically.
fn message(source_key: &str, field: &str, value: &Value, issued_at: u64) -> String {
    format!("onepass-fact-v1\n{source_key}\n{field}\n{value}\n{issued_at}")
}

/// Institution side: sign one fact.
pub fn sign_fact(key: &SigningKey, field: &str, value: &Value, issued_at: u64) -> SignedFact {
    let source_key = stellar_address(&key.verifying_key());
    let signature = key.sign(message(&source_key, field, value, issued_at).as_bytes());
    SignedFact {
        source_key,
        field: field.to_string(),
        value: value.clone(),
        issued_at,
        signature: BASE64.encode(signature.to_bytes()),
    }
}

/// Anyone's side: was this fact really signed by the key it names, and is it unaltered?
pub fn verify_fact(fact: &SignedFact) -> bool {
    let Ok(address) = stellar_strkey::ed25519::PublicKey::from_string(&fact.source_key) else {
        return false;
    };
    let Ok(public_key) = VerifyingKey::from_bytes(&address.0) else {
        return false;
    };
    let Ok(bytes) = BASE64.decode(&fact.signature) else {
        return false;
    };
    let Ok(signature) = Signature::from_slice(&bytes) else {
        return false;
    };
    let text = message(&fact.source_key, &fact.field, &fact.value, fact.issued_at);
    public_key.verify(text.as_bytes(), &signature).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn genuine_fact_passes() {
        let key = demo_key("northfield-university");
        let fact = sign_fact(&key, "is_student", &json!(true), 1_700_000_000);
        assert!(fact.source_key.starts_with('G'));
        assert!(verify_fact(&fact));
    }

    #[test]
    fn changed_value_fails() {
        let key = demo_key("maple-trust-bank");
        let mut fact = sign_fact(&key, "over_18", &json!(false), 1_700_000_000);
        fact.value = json!(true); // someone edits the fact after it was signed
        assert!(!verify_fact(&fact));
    }

    #[test]
    fn swapped_institution_fails() {
        let bank = demo_key("maple-trust-bank");
        let university = demo_key("northfield-university");
        let mut fact = sign_fact(&bank, "is_student", &json!(true), 1_700_000_000);
        // someone claims the university signed what the bank signed
        fact.source_key = stellar_address(&university.verifying_key());
        assert!(!verify_fact(&fact));
    }
}

/// A fingerprint (SHA-256 hash) of a set of signed facts, as 64 hex characters.
///
/// It is built from the signatures only, so it reveals nothing about the facts
/// themselves, yet anyone holding the same facts can recompute it and get the
/// same answer. This is what OnePass records on Stellar as a receipt.
pub fn fingerprint(facts: &[SignedFact]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"onepass-receipt-v1");
    for fact in facts {
        hasher.update(fact.signature.as_bytes());
    }
    hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()
}
