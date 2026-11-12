# OnePass

**One identity. Every door.**

OnePass gives a person one digital identity built from information that already
exists at their bank, university and other institutions. When a company needs
something (a name, an address, proof of being a student), it asks OnePass. The
person approves, OnePass fetches just that from the original source, checks it,
and hands it over stamped **"verified by OnePass"**. The company never has to
collect documents, and never learns which institutions the person uses.

## How one request works

1. A company asks OnePass for specific facts about a person.
2. The person gets a **notification**, sees exactly what is asked, chooses which
   of their institutions each fact comes from, and approves or denies.
3. Only after approval, OnePass fetches each fact from that institution.
4. OnePass checks the institution's digital signature and checks on **Stellar**
   that the institution is on the trusted registry. If not, it refuses.
5. OnePass stamps each fact with its own signature and delivers it. The
   institution's name is not passed on, and OnePass keeps no copy.
6. The company checks OnePass's stamp against OnePass's key on **Stellar**.
7. OnePass records a fingerprint of the verification on **Stellar** as a receipt.

## Why blockchain

Companies trust OnePass's stamp. Stellar is what makes that trust checkable
instead of blind. A smart contract (`contracts/registry`, written in Rust) holds:

- **The rulebook.** The public list of institutions OnePass is allowed to rely on.
  OnePass cannot quietly start accepting a fake university: every change to the
  list is a public, permanent transaction.
- **OnePass's own key.** A company checks a stamp against the key on Stellar, so
  nobody can forge "verified by OnePass", and a stolen key can be revoked in public.
- **Receipts.** Every verification leaves a timestamped fingerprint that can never
  be edited or deleted. In a dispute, anyone holding the facts can prove what
  OnePass vouched for and when. OnePass cannot backdate or deny it.

Only keys, names and fingerprints go on the blockchain. **No personal information.**

## How to run the demo

You need two terminals, both inside the project folder.

```bash
# Terminal 1: the stand-in institutions
cargo run -p onepass-sources

# Terminal 2: OnePass and the demo company pages
cargo run -p onepass-server
```

Then open http://127.0.0.1:4000 in a browser.

1. **Connect** Northfield University, Maple Trust Bank and CityHome Housing
   (login `alex` / `demo`).
2. **Open Harbour Talent** (a recruiter) in a new tab and send a background check.
   The company page waits.
3. Back on OnePass, a **notification** appears. Choose where each item comes from
   (for example, address from the bank or from the housing agency) and approve.
4. The recruiter's page shows the facts, each "Verified by OnePass", with no
   institution names, plus the Stellar receipt.
5. **The impostor:** disconnect Northfield University, connect **Diploma Express**,
   and request the student discount from Stride Sportswear. OnePass refuses,
   because Diploma Express is not on the Stellar registry.
6. **Tampering:** approve a request with "Demo: alter a fact in transit" ticked.
   The company rejects it, because OnePass's stamp no longer matches.
7. **Deny** a request, or untick an item: the company gets only what was approved.

`cargo test` runs the signature and smart-contract checks.

## Putting the registry on Stellar

One time, to deploy the smart contract to the Stellar testnet:

```bash
brew install stellar-cli          # or: cargo install --locked stellar-cli
bash scripts/setup-stellar.sh
```

The script deploys the contract, registers the three genuine institutions and
OnePass's key, and writes the contract id to `stellar-registry.txt`. Restart the
OnePass server and it uses Stellar; the pages link to the contract on a public
block explorer. Without that file the server falls back to a local copy of the
list, records no receipts, and says so on screen.

## The pieces

| Folder | What it is |
| --- | --- |
| `core/` | Signing and checking facts. Shared by the other two. |
| `sources/` | Stand-in institutions (port 4001). **All names and records are invented.** |
| `server/` | OnePass itself (port 4000): connect accounts, notifications, fetch, verify, stamp. Also serves the invented company pages. |
| `server/src/company.rs` | The checks a company runs on OnePass's stamp. |
| `server/src/registry.rs` | Reads the trusted list from the Stellar contract and records receipts. |
| `contracts/registry/` | The smart contract: trusted list, OnePass's key, receipts. |
| `scripts/setup-stellar.sh` | Deploys the contract to the Stellar testnet. |

### Stand-in institutions

| Institution | Kind | Facts it can provide |
| --- | --- | --- |
| Northfield University | university | name, student ID, student status, program, enrolment year |
| Maple Trust Bank | bank | name, date of birth, over 18, address, customer since |
| CityHome Housing | housing | name, address, tenant status, tenancy start |
| Diploma Express | "university" | An impostor: signs facts but is not on the registry |

Each institution signs every fact it hands out with its own key, so OnePass can tell a
genuine fact from an altered or forged one. OnePass keeps only an access token for each
connected institution, never a copy of the data.

## Still to build

- Shared control of the registry (several organisations approving changes, not one admin)
- Real institution integrations
- Phone notifications and a phone-style approval screen
- A company-side form for choosing which facts to request
