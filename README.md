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
5. OnePass stamps each fact with its own signature, made out to that company and
   that request only, and delivers it. The institution's name is not passed on,
   and OnePass wipes its copy 30 seconds later.
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

# Terminal 2: OnePass (landing page, person's page, requester page)
cargo run -p onepass-server
```

Then open http://127.0.0.1:4000 in a browser. There are three pages:

| Page | Address | Who it is for |
| --- | --- | --- |
| Landing page | `/` | Explains OnePass |
| Your OnePass | `/app` | The person whose information is shared |
| For companies | `/request` | The company or recruiter asking for it |

Open `/app` and `/request` in two tabs side by side.

1. **On `/app`, connect** Northfield University, Maple Trust Bank and CityHome Housing
   (the password is `demo`).
2. **On `/request`,** leave the company as Harbour Talent and the person as `alex`, pick
   "Background check" (or tick any items you like), and send the request.
3. **Back on `/app`,** the request appears within two seconds. Choose where each item
   comes from (address from the bank or the housing agency) and approve.
4. **On `/request`,** the facts arrive stamped "Verified by OnePass", with no institution
   names, followed by the Stellar receipt.
5. **The impostor:** disconnect Northfield University, connect **Diploma Express**, and
   request "Student discount". OnePass refuses, because Diploma Express is not on the
   Stellar registry. The company is only told that it could not be verified.
6. **Tampering:** approve a request with "Demo: alter a fact in transit" ticked. The
   company's check fails, because OnePass's stamp no longer matches.
7. **Passing it on:** on a verified result, click "Forward this result to Northgate
   Rentals". The second company rejects it: each stamp names the company and request it
   was issued for, and expires after ten minutes.
8. **Deletion:** about 30 seconds after delivery, the result shows "OnePass's copy of
   these facts: Deleted". The company keeps its copy; OnePass does not.
9. **Deny** a request, or untick an item: the company gets only what was approved.
10. **A second person:** switch "Signed in as" to Priya Nair on `/app`, and send a request
   for `priya`. Each person only sees requests about themselves.

`cargo test` runs the signature and smart-contract checks.

See [SECURITY.md](SECURITY.md) for what OnePass protects against and the prototype's known limits.

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
| `server/` | OnePass itself (port 4000): connect accounts, requests, fetch, verify, stamp. Serves the three web pages from `server/static/`. |
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
- Real sign-in for people and companies (the demo picks the person from a menu)
