//! The OnePass trusted-institution registry: a smart contract on Stellar.
//!
//! This is the one part of OnePass that lives on the blockchain. It stores a
//! public list answering "which signing keys belong to genuine institutions?"
//!
//!   * Anyone can READ the list (companies do, before accepting a fact).
//!   * Only the ADMIN can change it, and every change is a public, permanent
//!     transaction that anyone can audit.
//!   * No personal information is ever stored here: only institution keys,
//!     names and kinds.
//!
//! It also keeps RECEIPTS. Each time OnePass vouches for someone's information
//! it records a fingerprint (a hash) of what it vouched for. The fingerprint
//! reveals nothing about the person, but it is timestamped by the blockchain
//! and can never be altered or removed, so OnePass can later be held to it.
//!
//! A smart contract is a small program stored on the blockchain. Every Stellar
//! node runs the same code and agrees on the result, so nobody (not even
//! OnePass) can quietly show different companies a different list.

#![no_std] // contracts run in a tiny sandbox without Rust's standard library

use soroban_sdk::{contract, contractevent, contractimpl, contracttype, Address, BytesN, Env, Map, String, Vec};

/// One trusted institution.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct Institution {
    /// The key the institution signs facts with (Stellar "G..." format).
    pub public_key: String,
    /// Display name, e.g. "Maple Trust Bank".
    pub name: String,
    /// What it is: "university", "bank", "housing"...
    pub kind: String,
}

/// Published every time a receipt is recorded.
#[contractevent]
pub struct ReceiptRecorded {
    pub receipt: BytesN<32>,
}

/// Labels for the things the contract stores.
#[contracttype]
enum DataKey {
    Admin,
    Institutions,
    /// One entry per receipt: fingerprint -> time it was recorded.
    Receipt(BytesN<32>),
}

// Storage on Stellar expires unless it is renewed. Time is counted in
// "ledgers" (one roughly every 5 seconds). Each change keeps the registry
// alive for about 30 more days.
const KEEP_ALIVE_THRESHOLD: u32 = 100_000;
const KEEP_ALIVE_LEDGERS: u32 = 500_000;

#[contract]
pub struct Registry;

fn load(env: &Env) -> Map<String, Institution> {
    env.storage()
        .instance()
        .get(&DataKey::Institutions)
        .unwrap_or(Map::new(env))
}

fn require_admin(env: &Env) {
    let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
    // Fails the whole transaction unless it was signed by the admin's key.
    admin.require_auth();
}

#[contractimpl]
impl Registry {
    /// Runs once, when the contract is deployed. Records who may edit the list.
    pub fn __constructor(env: Env, admin: Address) {
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    /// Admin only: add an institution (or update it if the key is already listed).
    pub fn add(env: Env, public_key: String, name: String, kind: String) {
        require_admin(&env);
        let mut institutions = load(&env);
        institutions.set(public_key.clone(), Institution { public_key, name, kind });
        env.storage().instance().set(&DataKey::Institutions, &institutions);
        env.storage().instance().extend_ttl(KEEP_ALIVE_THRESHOLD, KEEP_ALIVE_LEDGERS);
    }

    /// Admin only: remove an institution, e.g. if its key was stolen.
    pub fn remove(env: Env, public_key: String) {
        require_admin(&env);
        let mut institutions = load(&env);
        institutions.remove(public_key);
        env.storage().instance().set(&DataKey::Institutions, &institutions);
    }

    /// Anyone: look up one key. Returns nothing if it is not trusted.
    pub fn get(env: Env, public_key: String) -> Option<Institution> {
        load(&env).get(public_key)
    }

    /// Anyone: the whole list.
    pub fn list(env: Env) -> Vec<Institution> {
        load(&env).values()
    }

    /// Admin only: record the fingerprint of one verification.
    /// The blockchain stamps it with the current time; it cannot be changed later.
    pub fn record(env: Env, receipt: BytesN<32>) {
        require_admin(&env);
        let key = DataKey::Receipt(receipt.clone());
        if env.storage().persistent().has(&key) {
            return; // already recorded: keep the original time
        }
        let now = env.ledger().timestamp();
        env.storage().persistent().set(&key, &now);
        env.storage().persistent().extend_ttl(&key, KEEP_ALIVE_THRESHOLD, KEEP_ALIVE_LEDGERS);
        // Announce it, so block explorers show each receipt as it happens.
        ReceiptRecorded { receipt }.publish(&env);
    }

    /// Anyone: when was this fingerprint recorded? (Nothing = never.)
    pub fn receipt_time(env: Env, receipt: BytesN<32>) -> Option<u64> {
        env.storage().persistent().get(&DataKey::Receipt(receipt))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    fn setup() -> (Env, RegistryClient<'static>) {
        let env = Env::default();
        let admin = Address::generate(&env);
        let id = env.register(Registry, (admin,));
        (env.clone(), RegistryClient::new(&env, &id))
    }

    #[test]
    fn admin_can_add_and_anyone_can_read() {
        let (env, registry) = setup();
        env.mock_all_auths(); // pretend the admin signed
        let key = String::from_str(&env, "GA4K6NUZFXL3USVVOGVU2ZQD2YAOLQNDS2KITPVYWZQ7V2I3XXD76Z7R");
        registry.add(&key, &String::from_str(&env, "Maple Trust Bank"), &String::from_str(&env, "bank"));

        let found = registry.get(&key).unwrap();
        assert_eq!(found.name, String::from_str(&env, "Maple Trust Bank"));
        assert_eq!(registry.list().len(), 1);
    }

    #[test]
    fn unknown_key_is_not_trusted() {
        let (env, registry) = setup();
        assert_eq!(registry.get(&String::from_str(&env, "GFAKE")), None);
    }

    #[test]
    fn removed_institution_is_no_longer_trusted() {
        let (env, registry) = setup();
        env.mock_all_auths();
        let key = String::from_str(&env, "GKEY");
        registry.add(&key, &String::from_str(&env, "Old Bank"), &String::from_str(&env, "bank"));
        registry.remove(&key);
        assert_eq!(registry.get(&key), None);
    }

    #[test]
    fn receipts_are_recorded_and_keep_their_first_time() {
        use soroban_sdk::testutils::Ledger as _;
        let (env, registry) = setup();
        env.mock_all_auths();
        let receipt = BytesN::from_array(&env, &[7u8; 32]);
        assert_eq!(registry.receipt_time(&receipt), None);

        env.ledger().set_timestamp(1_000);
        registry.record(&receipt);
        env.ledger().set_timestamp(2_000);
        registry.record(&receipt); // recording again must not move the time
        assert_eq!(registry.receipt_time(&receipt), Some(1_000));
    }

    #[test]
    #[should_panic]
    fn strangers_cannot_record_receipts() {
        let (env, registry) = setup();
        registry.record(&BytesN::from_array(&env, &[1u8; 32]));
    }

    #[test]
    #[should_panic]
    fn strangers_cannot_add() {
        let (env, registry) = setup();
        // No signature provided, so the admin check must fail.
        registry.add(&String::from_str(&env, "GEVIL"), &String::from_str(&env, "Fake Uni"), &String::from_str(&env, "university"));
    }
}
