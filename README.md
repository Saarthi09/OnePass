# OnePass

**One identity. Every door.**

OnePass gives a person one digital identity built from information that already
exists at their bank, university and other institutions. When a company needs
something (a name, an address, proof of being a student), OnePass fetches just
that from the original source, the person approves it, and the company can check
it is genuine using a registry on the Stellar blockchain.

## How to run the demo

You need two terminals, both inside the project folder.

```bash
# Terminal 1: the stand-in institutions
cargo run -p onepass-sources

# Terminal 2: OnePass and the demo company pages
cargo run -p onepass-server
```

Then open http://127.0.0.1:4000 in a browser.

1. **Connect** Northfield University and Maple Trust Bank (login `alex` / `demo`).
2. **Visit Stride Sportswear** and ask for the student discount. Approve the request.
   The shop receives one fact, "current student", and checks the university's signature.
3. **Visit Brightline Mobile** and sign up. Approve the request. The company receives
   name, address and date of birth, each signed by the bank. No form was filled in.
4. Repeat either one and tick **"Demo: alter a fact in transit"**. The company rejects it,
   because the signature no longer matches.
5. Repeat and untick a fact, or press **Deny**: the company gets only what was approved.

`cargo test` runs the signature checks.

## The pieces

| Folder | What it is |
| --- | --- |
| `core/` | Signing and checking facts. Shared by the other two. |
| `sources/` | Stand-in institutions (port 4001). **All names and records are invented.** |
| `server/` | OnePass itself (port 4000): connect accounts, fetch facts, user approval. Also serves the two invented company pages. |
| `server/src/company.rs` | The checks a company runs on what it receives. |
| `server/src/registry.rs` | The list of trusted institutions. Currently a local placeholder; the Stellar registry replaces it. |

### Stand-in institutions

| Institution | Kind | Facts it can provide |
| --- | --- | --- |
| Northfield University | university | name, student ID, student status, program, enrolment year |
| Maple Trust Bank | bank | name, date of birth, over 18, address, customer since |
| CityHome Housing | housing | name, address, tenant status, tenancy start |

Each institution signs every fact it hands out with its own key, so nobody can alter
a fact afterwards or pretend it came from somewhere else. OnePass keeps only an access
token for each connected institution, never a copy of the data.

## Still to build

- Stellar registry listing the institutions' public keys (replaces the local placeholder)
- Phone-style approval screen
