import { createHash, randomBytes } from "crypto";
import { Keypair } from "@stellar/stellar-sdk";
import type { ClaimValue, Credential, Disclosure, Presentation } from "./types";

/** JSON with keys sorted, so the same object always produces the same bytes to sign. */
function canonical(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  if (value && typeof value === "object") {
    const obj = value as Record<string, unknown>;
    return `{${Object.keys(obj)
      .sort()
      .map((k) => `${JSON.stringify(k)}:${canonical(obj[k])}`)
      .join(",")}}`;
  }
  return JSON.stringify(value);
}

/** Salted hash of one claim. The salt stops anyone guessing the value from the hash. */
export function hashClaim(name: string, d: Disclosure): string {
  return createHash("sha256")
    .update(`${d.salt}:${name}:${JSON.stringify(d.value)}`)
    .digest("hex");
}

function signingBytes(c: Omit<Credential, "signature">): Buffer {
  const { type, issuer, issuerName, issuedAt, claimHashes } = c;
  return Buffer.from(canonical({ type, issuer, issuerName, issuedAt, claimHashes }));
}

/** Issuer side: hash every claim separately, then sign the list of hashes. */
export function issueCredential(
  issuerKey: Keypair,
  issuerName: string,
  type: string,
  claims: Record<string, ClaimValue>
): { credential: Credential; disclosures: Record<string, Disclosure> } {
  const disclosures: Record<string, Disclosure> = {};
  const claimHashes: Record<string, string> = {};
  for (const [name, value] of Object.entries(claims)) {
    disclosures[name] = { value, salt: randomBytes(16).toString("hex") };
    claimHashes[name] = hashClaim(name, disclosures[name]);
  }
  const unsigned = {
    type,
    issuer: issuerKey.publicKey(),
    issuerName,
    issuedAt: new Date().toISOString(),
    claimHashes,
  };
  const signature = Buffer.from(issuerKey.sign(signingBytes(unsigned))).toString("base64");
  return { credential: { ...unsigned, signature }, disclosures };
}

/** Was this credential really signed by the public key it names? */
export function verifySignature(c: Credential): boolean {
  try {
    return Keypair.fromPublicKey(c.issuer).verify(
      signingBytes(c),
      Buffer.from(c.signature, "base64")
    );
  } catch {
    return false;
  }
}

/** Do the revealed values match the hashes the issuer signed? Returns the bad claim names. */
export function mismatchedClaims(p: Presentation): string[] {
  return Object.entries(p.revealed)
    .filter(([name, d]) => p.credential.claimHashes[name] !== hashClaim(name, d))
    .map(([name]) => name);
}
