# Security and privacy in OnePass

OnePass is a prototype. This page says what it protects against today, what it
does not, and what would change before real use.

## What OnePass protects against

| Threat | What OnePass does | See it in the demo |
| --- | --- | --- |
| A fact is altered on its way to the company | Every fact carries a digital signature. Changing one character breaks it. | Tick "Demo: alter a fact in transit" before approving |
| A fake institution vouches for someone | OnePass only accepts institutions listed on the Stellar registry | Connect Diploma Express and request student status |
| Someone forges "Verified by OnePass" | Companies check the stamp against OnePass's key published on Stellar | "OnePass signing key: On the registry" on every result |
| A scammer asks for your information using a real company's name | Registered companies sign each request, and OnePass checks the signature against the company's key on the Stellar registry. Anything else is flagged to the person | On the requester page: send as Harbour Talent ("Verified company"), remove it from the registry and send again ("Unverified requester"), or tick "Demo: send this as a scammer" |
| A company passes its result on, or reuses it later | Each stamp names the company and request it was issued for, and expires after 10 minutes | "Forward this result to Northgate Rentals" on a verified result |
| Information is shared without consent | Nothing is fetched from an institution until the person approves | Requests wait in the person's notifications |
| A company learns where you bank or study | Institution names are never passed on | Results say "Sources are not disclosed" |
| OnePass becomes a store of personal data | OnePass keeps access tokens, not data, and wipes delivered facts 30 seconds after delivery | "OnePass's copy of these facts: Deleted" on the result |
| OnePass denies or backdates a verification | A fingerprint of every verification is recorded on Stellar, permanently | "Receipt recorded" on the result |
| Personal data leaks from the blockchain | Only keys, institution and company names, and fingerprints are written to Stellar | Open the contract on the block explorer |

## What is on the blockchain, and what is not

**On Stellar:** the public keys and names of trusted institutions and registered
companies, OnePass's own public key, and a SHA-256 fingerprint for each verification.

**Never on Stellar:** names, addresses, dates of birth, which person a receipt
belongs to, which company asked, or which institution supplied a fact.

A fingerprint is built from OnePass's signatures, not from the facts, so it
cannot be reversed or guessed back into personal information.

## Known limits of the prototype

These are deliberate simplifications for a demo, not solved problems.

1. **Only one company is registered.** Harbour Talent has a key on the registry
   and its requests are verified. Any other name is sent unsigned and shown with
   an "Unverified requester" warning. In the demo the OnePass server holds
   Harbour Talent's key and signs for it, and the add/remove button on the
   requester page stands in for the registry admin. Fix: each company signs on
   its own servers, and is added only after an onboarding check.
2. **No real sign-in.** The demo picks the person from a menu. Fix: passkeys.
3. **OnePass handles institution passwords.** They are passed straight through
   and not saved, but OnePass still sees them. Fix: send the person to the
   institution's own sign-in page (OAuth), so OnePass only ever gets a token.
4. **Demo keys are predictable.** Signing keys are derived from names so the
   demo is repeatable. Fix: random keys held in secure hardware.
5. **OnePass sees facts while stamping them.** Fix: encrypt each fact to the
   company's key, or prove a claim ("over 18") without revealing the value.
6. **One admin controls the registry.** Fix: require several independent
   organisations to approve each change, using Stellar's multi-signature accounts.
7. **Everything runs on one computer over plain HTTP.** A deployment would use
   HTTPS everywhere and separate servers for each party.

## Reporting a problem

This is a student prototype for the Blockchain Innovation Challenge. If you find
a flaw, please open an issue on the repository.
