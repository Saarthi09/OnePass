# OnePass

**One identity. Every door.**

OnePass lets a person prove facts about themselves (for example, "I am a student") to any
company without re-submitting documents and without anyone holding a central database of
personal data.

## How it works

1. **University (issuer)** signs a credential for the student. Each claim is hashed
   separately with a random salt, and the university signs the list of hashes.
2. **Wallet (user)** keeps the credential in the user's own browser.
3. **Company (verifier)** asks for specific claims, e.g. `isStudent` and `university`.
4. The user approves exactly which claims to share.
5. The company checks:
   - the signature matches the issuer's key,
   - the issuer's key is in the **trusted-issuer registry on Stellar**,
   - the shared values match the signed hashes,
   - everything requested was shared.

Because each claim is hashed separately, a user can prove "over 18" without revealing
their date of birth.

## Why Stellar

The registry of trusted issuers is a Stellar account whose data entries map
`issuer public key -> issuer name`. Anyone can read it, nobody can quietly edit it, and
companies can verify a credential without contacting OnePass or the university.
Personal information is never written to the blockchain.

## Run it

```bash
npm install
npm run dev          # http://localhost:3000
```

With no configuration the app runs in **mock mode** (a local stand-in for the registry).

To use the real registry on Stellar testnet:

```bash
npm run setup:registry   # creates the registry account and writes .env.local
npm run dev
```

Demo order: **University** → **Company** → approve in **Wallet**.

```bash
npm test             # checks signing, tampering and untrusted-issuer cases
```

## Project layout

| Path | What it does |
| --- | --- |
| `src/lib/credential.ts` | Issue, sign and check credentials |
| `src/lib/registry.ts` | Looks up issuers in the Stellar registry |
| `src/lib/verify.ts` | The checks a company runs |
| `src/app/issuer`, `wallet`, `verifier` | The three demo screens |
| `src/app/api` | Issue, request and present endpoints |
| `scripts/setup-registry.mjs` | Creates the on-chain registry (testnet) |

## Not done yet

- Proof that the person presenting a credential is the person it was issued to
- Revoking credentials
- Real university or bank integrations (the issuer here is a demo)
