import { mismatchedClaims, verifySignature } from "./credential";
import { lookupIssuer } from "./registry";
import type { ClaimValue, Presentation, VerificationResult } from "./types";

/** Verifier side: the four checks a company runs on what the user shared. */
export async function verifyPresentation(
  p: Presentation,
  requiredClaims: string[]
): Promise<VerificationResult> {
  const checks: VerificationResult["checks"] = [];

  const signed = verifySignature(p.credential);
  checks.push({ label: "Signature matches the issuer's key", passed: signed });

  let issuerName: string | undefined;
  let registrySource: string | undefined;
  try {
    const lookup = await lookupIssuer(p.credential.issuer);
    issuerName = lookup.name;
    registrySource = lookup.source;
    checks.push({
      label: "Issuer is in the trusted registry",
      passed: lookup.trusted,
      detail: lookup.trusted ? `${lookup.name} (${lookup.source})` : lookup.source,
    });
  } catch (e) {
    checks.push({
      label: "Issuer is in the trusted registry",
      passed: false,
      detail: `Registry lookup failed: ${(e as Error).message}`,
    });
  }

  const bad = mismatchedClaims(p);
  checks.push({
    label: "Shared values match what the issuer signed",
    passed: bad.length === 0,
    detail: bad.length ? `Tampered: ${bad.join(", ")}` : undefined,
  });

  const missing = requiredClaims.filter((c) => !(c in p.revealed));
  checks.push({
    label: "All requested information was shared",
    passed: missing.length === 0,
    detail: missing.length ? `Missing: ${missing.join(", ")}` : undefined,
  });

  const ok = checks.every((c) => c.passed);
  const claims: Record<string, ClaimValue> = {};
  if (ok) for (const [k, d] of Object.entries(p.revealed)) claims[k] = d.value;
  return { ok, checks, claims, issuerName, registrySource };
}
