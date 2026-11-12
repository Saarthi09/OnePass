// Shared shapes used by the issuer, the wallet and the verifier.

export type ClaimValue = string | number | boolean;

/** What the issuer signs. It contains only hashes of the claims, never the values. */
export interface Credential {
  type: string;
  issuer: string; // issuer's Stellar public key (G...)
  issuerName: string;
  issuedAt: string;
  claimHashes: Record<string, string>;
  signature: string; // base64 ed25519 signature by the issuer
}

/** The secret half the user keeps: the real value and a random salt per claim. */
export interface Disclosure {
  value: ClaimValue;
  salt: string;
}

/** What sits in the user's wallet. */
export interface WalletEntry {
  credential: Credential;
  disclosures: Record<string, Disclosure>;
}

/** What the user sends to a company: the credential plus only the claims they chose. */
export interface Presentation {
  credential: Credential;
  revealed: Record<string, Disclosure>;
}

export interface VerificationResult {
  ok: boolean;
  checks: { label: string; passed: boolean; detail?: string }[];
  claims: Record<string, ClaimValue>;
  issuerName?: string;
  registrySource?: string;
}

export interface VerificationRequest {
  id: string;
  verifier: string;
  requiredClaims: string[];
  status: "pending" | "verified" | "rejected";
  result?: VerificationResult;
}
