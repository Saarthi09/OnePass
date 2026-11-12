// One-time setup: creates the trusted-issuer registry on Stellar testnet.
//   1. Makes a registry account and funds it with Friendbot (free test XLM).
//   2. Makes a signing key for the demo university.
//   3. Writes "issuer public key -> issuer name" as a data entry on the registry account.
//   4. Saves the keys to .env.local so the app uses the real registry.
import { writeFileSync } from "fs";
import {
  BASE_FEE,
  Horizon,
  Keypair,
  Networks,
  Operation,
  TransactionBuilder,
} from "@stellar/stellar-sdk";

const HORIZON_URL = "https://horizon-testnet.stellar.org";
const ISSUER_NAME = "University of Waterloo";

const registry = Keypair.random();
const issuer = Keypair.random();

console.log("Funding registry account with Friendbot…");
const funded = await fetch(`https://friendbot.stellar.org?addr=${registry.publicKey()}`);
if (!funded.ok) throw new Error(`Friendbot failed: ${funded.status}`);

const server = new Horizon.Server(HORIZON_URL);
const account = await server.loadAccount(registry.publicKey());

console.log("Registering issuer on-chain…");
const tx = new TransactionBuilder(account, {
  fee: BASE_FEE,
  networkPassphrase: Networks.TESTNET,
})
  .addOperation(Operation.manageData({ name: issuer.publicKey(), value: ISSUER_NAME }))
  .setTimeout(60)
  .build();
tx.sign(registry);
const result = await server.submitTransaction(tx);

writeFileSync(
  ".env.local",
  [
    `ISSUER_SECRET=${issuer.secret()}`,
    `REGISTRY_ACCOUNT=${registry.publicKey()}`,
    `REGISTRY_SECRET=${registry.secret()}`,
    `HORIZON_URL=${HORIZON_URL}`,
    "",
  ].join("\n")
);

console.log("\nDone. Saved keys to .env.local");
console.log(`Registry account: ${registry.publicKey()}`);
console.log(`Issuer key:       ${issuer.publicKey()}`);
console.log(`Transaction:      https://stellar.expert/explorer/testnet/tx/${result.hash}`);
