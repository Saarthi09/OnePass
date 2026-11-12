import { Horizon } from "@stellar/stellar-sdk";
import { getIssuerKeypair, ISSUER_NAME } from "./issuer";

export interface IssuerLookup {
  trusted: boolean;
  name?: string;
  source: "stellar-testnet" | "mock";
}

/**
 * The trusted-issuer registry.
 *
 * On Stellar, the registry is one account whose data entries map
 *   issuer public key -> issuer name.
 * Anyone can read it, and only the registry account's signers can change it.
 * No personal information is ever written here.
 */
export async function lookupIssuer(issuerPublicKey: string): Promise<IssuerLookup> {
  const registryAccount = process.env.REGISTRY_ACCOUNT;

  if (!registryAccount) {
    // Mock mode: trust only the built-in demo issuer.
    const trusted = issuerPublicKey === getIssuerKeypair().publicKey();
    return { trusted, name: trusted ? ISSUER_NAME : undefined, source: "mock" };
  }

  const server = new Horizon.Server(
    process.env.HORIZON_URL ?? "https://horizon-testnet.stellar.org"
  );
  const account = await server.loadAccount(registryAccount);
  const entry = account.data_attr[issuerPublicKey]; // base64 value, or undefined
  if (!entry) return { trusted: false, source: "stellar-testnet" };
  return {
    trusted: true,
    name: Buffer.from(entry, "base64").toString("utf8"),
    source: "stellar-testnet",
  };
}
