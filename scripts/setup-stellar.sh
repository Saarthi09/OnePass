#!/bin/bash
# One-time setup: put the OnePass trusted-institution registry on the Stellar testnet.
#
# Run from the project folder:   bash scripts/setup-stellar.sh
#
# What it does:
#   1. Creates a Stellar account for the registry's admin and gives it free test money.
#   2. Builds the smart contract (contracts/registry) into a WebAssembly file.
#   3. Deploys the contract to the Stellar testnet.
#   4. Adds the three genuine demo institutions, and OnePass's own verifier
#      key, to the on-chain list.
#   5. Saves the contract's id in stellar-registry.txt, which switches the
#      OnePass server from its local fallback to the real registry.
#
# "Testnet" is Stellar's practice network: real blockchain, play money.

set -e  # stop at the first error

ADMIN=onepass-admin
NETWORK=testnet
WASM=target/wasm32v1-none/release/onepass_registry.wasm

if ! command -v stellar >/dev/null; then
  echo "The Stellar CLI is not installed. Install it first (see README), then run this again."
  exit 1
fi

echo "== 1/5  Admin account =="
if stellar keys address "$ADMIN" >/dev/null 2>&1; then
  echo "Using existing identity '$ADMIN'."
else
  stellar keys generate "$ADMIN" --network "$NETWORK"
fi
# Ask Friendbot (Stellar's test-money tap) to fund it. Harmless if already funded.
stellar keys fund "$ADMIN" --network "$NETWORK" || true
echo "Admin address: $(stellar keys address "$ADMIN")"

echo "== 2/5  Build the contract =="
rustup target add wasm32v1-none
stellar contract build --package onepass-registry

echo "== 3/5  Deploy to Stellar testnet =="
# Everything after the lone "--" is passed to the contract's constructor.
CONTRACT_ID=$(stellar contract deploy \
  --wasm "$WASM" \
  --source-account "$ADMIN" \
  --network "$NETWORK" \
  -- --admin "$ADMIN")
echo "Contract id: $CONTRACT_ID"

echo "== 4/5  Register the genuine institutions and OnePass's key =="
add() {
  stellar contract invoke --id "$CONTRACT_ID" --source-account "$ADMIN" --network "$NETWORK" \
    -- add --public_key "$1" --name "$2" --kind "$3"
  echo "  added $2"
}
add GCKSFO63Z4OVXXULTTFQJNAKBSYTEI5VIKZISTUCITO77OCUO7VPZA4W "Northfield University" university
add GA4K6NUZFXL3USVVOGVU2ZQD2YAOLQNDS2KITPVYWZQ7V2I3XXD76Z7R "Maple Trust Bank" bank
add GAGUIDWAQY6FV3AF5WMGHW2TYQOQH6PWYMA33BM5MASB7HQ4UFKFNFWR "CityHome Housing" housing
# "Diploma Express" is deliberately NOT added: it is the impostor in the demo.
# OnePass's own key, so companies can check that a "verified by OnePass" stamp is real.
add GCYK7UIOCLXXHFAJSIPZ56TC5LPDLIFL25R6TK3W2ZSIGRYG4DZBF4ME "OnePass" verifier
# A registered company. OnePass checks each request's signature against this key
# before telling the person the request really comes from Harbour Talent.
add GDT4YT2YQDWX65RZEZFTPSCZWJG5ZNEJLAARBFQLRNY75WADENDLQQ6A "Harbour Talent" company

echo "== 5/5  Save the contract id =="
echo "$CONTRACT_ID" > stellar-registry.txt

echo
echo "Done. The registry now lives on Stellar:"
echo "  https://stellar.expert/explorer/testnet/contract/$CONTRACT_ID"
echo "What it currently holds:"
stellar contract invoke --id "$CONTRACT_ID" --source-account "$ADMIN" --network "$NETWORK" -- list
echo
echo "Restart the OnePass server (Ctrl+C, then: cargo run -p onepass-server) to use it."
