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
6. **The impostor:** disconnect Northfield University, connect **Diploma Express**, and ask
   for the student discount again. Its signature is valid, but its key is not on the Stellar
   registry, so the shop rejects it. This is what the blockchain is for.

`cargo test` runs the signature checks.

## The Stellar registry

Which institutions can be trusted? That list is a **smart contract on Stellar**
(`contracts/registry`, written in Rust):

- anyone can read it, so a company can check a signer without asking OnePass;
- only the admin key can change it, and every change is a public, permanent transaction;
- it holds institution keys and names only. **No personal information goes on the blockchain.**

Put it on the Stellar testnet (one time):

```bash
brew install stellar-cli          # or: cargo install --locked stellar-cli
bash scripts/setup-stellar.sh
```

The script deploys the contract, registers the three genuine institutions, and writes the
contract id to `stellar-registry.txt`. Restart the OnePass server and it reads the list from
Stellar; the pages link to the contract on a public block explorer. Without that file the
server falls back to a local copy of the list and says so on screen.

## The pieces

| Folder | What it is |
| --- | --- |
| `core/` | Signing and checking facts. Shared by the other two. |
| `sources/` | Stand-in institutions (port 4001). **All names and records are invented.** |
| `server/` | OnePass itself (port 4000): connect accounts, fetch facts, user approval. Also serves the two invented company pages. |
| `server/src/company.rs` | The checks a company runs on what it receives. |
| `server/src/registry.rs` | Reads the list of trusted institutions from the Stellar contract. |
| `contracts/registry/` | The smart contract that holds that list on Stellar. |
| `scripts/setup-stellar.sh` | Deploys the contract to the Stellar testnet. |

### Stand-in institutions

| Institution | Kind | Facts it can provide |
| --- | --- | --- |
| Northfield University | university | name, student ID, student status, program, enrolment year |
| Maple Trust Bank | bank | name, date of birth, over 18, address, customer since |
| CityHome Housing | housing | name, address, tenant status, tenancy start |
| Diploma Express | "university" | An impostor: signs facts but is not on the registry |

Each institution signs every fact it hands out with its own key, so nobody can alter
a fact afterwards or pretend it came from somewhere else. OnePass keeps only an access
token for each connected institution, never a copy of the data.

## Still to build

- Shared control of the registry (several organisations approving changes, not one admin)
- Real institution integrations
- Phone-style approval screen
