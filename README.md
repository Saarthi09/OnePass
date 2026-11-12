# OnePass

**One identity. Every door.**

OnePass gives a person one digital identity built from information that already
exists at their bank, university and other institutions. When a company needs
something (a name, an address, proof of being a student), OnePass fetches just
that from the original source, the person approves it, and the company can check
it is genuine using a registry on the Stellar blockchain.

## What is built so far

### `sources/` — stand-in institutions

OnePass cannot connect to real institutions for this project, so three invented
ones play their part. **All names and records are fictional.**

| Institution | Kind | Facts it can provide |
| --- | --- | --- |
| Northfield University | university | name, student ID, student status, program, enrolment year |
| Maple Trust Bank | bank | name, date of birth, over 18, address, customer since |
| CityHome Housing | housing | name, address, tenant status, tenancy start |

Demo logins: `alex` / `demo` and `priya` / `demo`.

Each institution signs every fact it hands out with its own key, so nobody can
alter a fact afterwards or pretend it came from somewhere else.

```bash
cargo run -p onepass-sources     # starts on http://127.0.0.1:4001
cargo test                       # checks that tampered facts are rejected
```

Try it:

```bash
# 1. Sign in to the bank (returns a token)
curl -X POST http://127.0.0.1:4001/sources/maple-trust-bank/login \
  -H 'content-type: application/json' -d '{"username":"alex","password":"demo"}'

# 2. Ask for signed facts, using that token
curl 'http://127.0.0.1:4001/sources/maple-trust-bank/facts?fields=full_name,address' \
  -H 'Authorization: Bearer <token>'
```

## Still to build

- Stellar registry listing the institutions' public keys
- OnePass server: connect accounts, handle company requests, user approval
- Demo company site and the web pages
