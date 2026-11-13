#!/bin/sh
# Starts the OnePass demo inside the container.
#
# Stellar mode needs the registry admin's secret key (the "onepass-admin"
# identity that scripts/setup-stellar.sh created). Set it as the environment
# variable ONEPASS_ADMIN_SECRET on the hosting service. Without it, the server
# runs in local mode: same demo, but no on-chain lookups or receipts.
set -e

if [ -n "$ONEPASS_ADMIN_SECRET" ]; then
  mkdir -p "$HOME/.config/stellar/identity"
  printf 'secret_key = "%s"\n' "$ONEPASS_ADMIN_SECRET" > "$HOME/.config/stellar/identity/onepass-admin.toml"
  echo "Stellar mode: using contract $(cat stellar-registry.txt)"
else
  rm -f stellar-registry.txt
  echo "ONEPASS_ADMIN_SECRET not set: running with the local registry (no Stellar)."
fi

# The stand-in institutions, reachable only inside the container.
onepass-sources &

# OnePass itself, on the port the host gives us.
exec onepass-server
