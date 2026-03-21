#!/usr/bin/env bash
set -euo pipefail

# Shared defaults for demo scripts.
# Source this file before running demo scripts if you want these defaults in your shell:
#   source demo/env.sh

export ZINC_CLI_NETWORK="${ZINC_CLI_NETWORK:-regtest}"
export ZINC_CLI_SCHEME="${ZINC_CLI_SCHEME:-dual}"
export ZINC_CLI_ESPLORA_URL="${ZINC_CLI_ESPLORA_URL:-https://regtest.exittheloop.com/api}"
export ZINC_CLI_ORD_URL="${ZINC_CLI_ORD_URL:-https://ord-regtest.exittheloop.com}"
export ZINC_WALLET_PASSWORD="${ZINC_WALLET_PASSWORD:-demo_password_123}"
