// Quick checks for the credential logic. Run with: npm test
import assert from "assert";
import { Keypair } from "@stellar/stellar-sdk";
import { issueCredential } from "../src/lib/credential";
import { getIssuerKeypair, ISSUER_NAME } from "../src/lib/issuer";
import { verifyPresentation } from "../src/lib/verify";

async function main() {
  const claims = { name: "Test Student", isStudent: true, university: ISSUER_NAME, over18: true };
  const { credential, disclosures } = issueCredential(getIssuerKeypair(), ISSUER_NAME, "StudentCredential", claims);
  const required = ["isStudent", "university"];
  const reveal = (names: string[]) => Object.fromEntries(names.map((n) => [n, disclosures[n]]));

  // 1. Honest share of only the requested claims passes, and nothing else leaks.
  let r = await verifyPresentation({ credential, revealed: reveal(required) }, required);
  assert.equal(r.ok, true);
  assert.deepEqual(Object.keys(r.claims).sort(), ["isStudent", "university"]);

  // 2. Changing a value is caught.
  const tampered = reveal(required);
  tampered.university = { ...tampered.university, value: "Fake University" };
  r = await verifyPresentation({ credential, revealed: tampered }, required);
  assert.equal(r.ok, false);

  // 3. A credential signed by an unknown issuer is rejected by the registry.
  const fake = issueCredential(Keypair.random(), ISSUER_NAME, "StudentCredential", claims);
  r = await verifyPresentation(
    { credential: fake.credential, revealed: { isStudent: fake.disclosures.isStudent, university: fake.disclosures.university } },
    required
  );
  assert.equal(r.ok, false);
  assert.equal(r.checks[0].passed, true); // signature itself is valid…
  assert.equal(r.checks[1].passed, false); // …but the issuer is not trusted

  // 4. Editing the signed part breaks the signature.
  r = await verifyPresentation(
    { credential: { ...credential, issuerName: "Someone Else" }, revealed: reveal(required) },
    required
  );
  assert.equal(r.checks[0].passed, false);

  // 5. Leaving out a requested claim fails.
  r = await verifyPresentation({ credential, revealed: reveal(["isStudent"]) }, required);
  assert.equal(r.ok, false);

  console.log("All 5 credential checks passed.");
}

main();
