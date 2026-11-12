import { Keypair } from "@stellar/stellar-sdk";

export const ISSUER_NAME = "University of Waterloo";

const g = globalThis as unknown as { __devIssuer?: Keypair };

/** The demo university's signing key. Uses a throwaway key if none is configured. */
export function getIssuerKeypair(): Keypair {
  if (process.env.ISSUER_SECRET) return Keypair.fromSecret(process.env.ISSUER_SECRET);
  if (!g.__devIssuer) g.__devIssuer = Keypair.random();
  return g.__devIssuer;
}
