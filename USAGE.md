# zinc-cli Usage (Human + Agent)

This guide is task-oriented. For strict response contracts, see `COMMAND_CONTRACT_V1.md` and `SCHEMAS.md`.

## 1) Command Shape

Global flags can appear before or after command tokens:

```bash
zinc-cli [global flags] <command> [command flags]
```

`dashboard` and the interactive setup wizard are available only in builds compiled with the `ui` feature (for example: `cargo install zinc-wallet-cli --features ui`).
With `ui` enabled, the basic dashboard shows account balance, inscriptions, and ordinals/payment addresses for each account.

Useful globals:

- `--agent` machine output mode (returns structured JSON)
- `--profile <name>` select profile (default: `default`)
- `--data-dir <path>` override data root
- `--password <value>`
- `--password-env <ENV_NAME>` (default env: `ZINC_WALLET_PASSWORD`)
- `--password-stdin`
- `--reveal` show mnemonic fields in `--agent` mode, and on `wallet import`
- `--correlation-id <id>` set a stable workflow/request identifier
- `--log-json` emit structured lifecycle logs to stderr (`command_start|command_finish|command_error`)
- `--idempotency-key <key>` de-duplicate mutating commands for retry-safe automation
- `--network-timeout-secs <n>` timeout for remote calls (default: `30`)
- `--network-retries <n>` retry count for transient network failures/timeouts (default: `0`)
- `--policy-mode warn|strict` transaction safety behavior (default: `warn`)
- `--thumb` force inscription thumbnails on
- `--no-thumb` disable inscription thumbnails

Environment defaults (optional):

- `ZINC_CLI_PROFILE`
- `ZINC_CLI_DATA_DIR`
- `ZINC_CLI_PASSWORD_ENV`
- `ZINC_CLI_OUTPUT` (`human|agent`)
- `ZINC_CLI_NETWORK`
- `ZINC_CLI_SCHEME`
- `ZINC_CLI_ESPLORA_URL`
- `ZINC_CLI_ORD_URL`
- `ZINC_CLI_CORRELATION_ID`
- `ZINC_CLI_LOG_JSON` (`1|true|yes|on`)
- `ZINC_CLI_IDEMPOTENCY_KEY`
- `ZINC_CLI_NETWORK_TIMEOUT_SECS`
- `ZINC_CLI_NETWORK_RETRIES`
- `ZINC_CLI_POLICY_MODE` (`warn|strict`)

Inspect effective config:

```bash
zinc-cli --agent config show
```

Persist config defaults:

```bash
zinc-cli setup
zinc-cli setup --profile bot-a --data-dir /var/lib/zinc --password-env BOT_PASS
zinc-cli config set network signet
zinc-cli config set scheme unified
```

`zinc-cli setup` starts an interactive wizard when run in a terminal and can initialize a wallet profile at the end (generate new or restore existing mnemonic).

Password precedence:

1. `--password`
2. `--password-stdin`
3. `--password-env`

## 2) Human Quick Start

Set a default password env once:

```bash
export ZINC_WALLET_PASSWORD='your-wallet-password'
```

Initialize wallet:

```bash
zinc-cli wallet init --network signet --overwrite
```

Show wallet info:

```bash
zinc-cli wallet info
```

Reveal seed phrase (sensitive):

```bash
zinc-cli --yes --agent wallet reveal-mnemonic
```

Sync and check balance:

```bash
zinc-cli sync chain
zinc-cli sync ordinals
zinc-cli balance
zinc-cli inscription list
```

Get addresses:

```bash
zinc-cli address taproot
zinc-cli address payment
```



## 3) Agent Mode (Recommended)

Use `--agent` and parse stdout as one JSON object.

```bash
zinc-cli --agent wallet info
```

Expected envelope:

```json
{
  "ok": true,
  "schema_version": "1.0",
  "command": "wallet info"
}
```

Error envelope:

```json
{
  "ok": false,
  "schema_version": "1.0",
  "command": "wallet info",
  "error": {
    "type": "config",
    "message": "failed to read profile: ...",
    "exit_code": 10
  }
}
```

Bash error handling pattern:

```bash
out="$(zinc-cli --agent wallet info)"
ok="$(printf '%s' "$out" | jq -r '.ok')"
if [ "$ok" != "true" ]; then
  printf '%s\n' "$out" | jq -r '.error.type + ": " + .error.message' >&2
  exit "$(printf '%s' "$out" | jq -r '.error.exit_code')"
fi
```

## 4) Agent Reliability Controls

Use strict policy, idempotency, and retry controls together:

```bash
CID="agent-run-42"
zinc-cli --agent \
  --correlation-id "$CID" \
  --log-json \
  --idempotency-key "send-0001" \
  --network-timeout-secs 20 \
  --network-retries 2 \
  --policy-mode strict \
  psbt broadcast --psbt-file /tmp/send.signed.psbt
```

Behavior:
- repeated call with the same `--idempotency-key` + same mutating payload replays cached success
- reusing the same key with a different mutating payload returns `error.type=invalid`
- in `--policy-mode strict`, risky/unknown PSBT policy outcomes are blocked with `error.type=policy`

## 5) PSBT Flow

Create:

```bash
zinc-cli --agent psbt create \
  --to <address> --amount-sats 10000 --fee-rate 2 --out-file /tmp/send.psbt
```

Analyze:

```bash
zinc-cli --agent psbt analyze --psbt-file /tmp/send.psbt
```

Sign:

```bash
zinc-cli --agent psbt sign \
  --psbt-file /tmp/send.psbt --finalize --out-file /tmp/send.signed.psbt
```

Broadcast:

```bash
zinc-cli --agent psbt broadcast --psbt-file /tmp/send.signed.psbt
```

Rules:

- For `psbt analyze/sign/broadcast`, exactly one of `--psbt`, `--psbt-file`, `--psbt-stdin` is required.
- `--password-stdin` cannot be combined with `--psbt-stdin` in one invocation.

## 6) Offer Commands (Nostr + Ord, Advanced)

`offer` is callable directly (`zinc-cli offer ...`) but intentionally hidden from top-level `zinc-cli --help`.

Create an ord-compatible buyer offer PSBT and a relay-ready offer envelope:

```bash
zinc-cli --agent --ord-url https://ord.example offer create \
  --inscription <inscription-id> \
  --amount 100000 \
  --fee-rate 1 \
  --expires-in-secs 3600 \
  --seller-payout-address <seller-payment-address> \
  --publisher-pubkey-hex <xonly-pubkey-hex> \
  --offer-out-file /tmp/offer.json \
  --psbt-out-file /tmp/offer.psbt
```

Create and immediately submit the PSBT to ord:

```bash
zinc-cli --agent --ord-url https://ord.example offer create \
  --inscription <inscription-id> \
  --amount 100000 \
  --fee-rate 1 \
  --submit-ord
```

Publish a signed offer event to one or more relays:

```bash
zinc-cli --agent offer publish \
  --offer-json '{"version":1,"seller_pubkey_hex":"<xonly-pubkey-hex>","network":"regtest","inscription_id":"<inscription-id>","seller_outpoint":"<txid:vout>","ask_sats":100000,"fee_rate_sat_vb":1,"psbt_base64":"<base64-psbt>","created_at_unix":1710000000,"expires_at_unix":1710003600,"nonce":42}' \
  --secret-key-hex <seller-secret-key-hex> \
  --relay wss://nostr.example
```

Human-focused, glanceable offer output (great for demos):

```bash
zinc-cli --ord-url https://ord.example --thumb offer create \
  --inscription <inscription-id> \
  --amount 100000 \
  --fee-rate 1
```

```bash
zinc-cli --ord-url https://ord.example --thumb offer discover \
  --relay wss://nostr.example
```

```bash
zinc-cli --ord-url https://ord.example --thumb offer accept \
  --offer-file /tmp/offer.json
```

Discover offers from one or more relays:

```bash
zinc-cli --agent offer discover \
  --relay wss://nostr.example \
  --limit 256 \
  --timeout-ms 5000
```

Accept an offer from an offer envelope (sign seller input and optionally broadcast):

```bash
zinc-cli --agent offer accept \
  --offer-file /tmp/offer.json \
  --expect-inscription <inscription-id> \
  --expect-ask-sats 100000
```

Dry run acceptance checks (no broadcast):

```bash
zinc-cli --agent offer accept \
  --offer-file /tmp/offer.json \
  --dry-run
```

Submit an offer PSBT to ord:

```bash
zinc-cli --agent --ord-url https://ord.example \
  offer submit-ord --psbt-file /tmp/offer.psbt
```

List offer PSBTs from ord:

```bash
zinc-cli --agent --ord-url https://ord.example \
  offer list-ord
```

Rules:

- For `offer publish`, exactly one of `--offer-json`, `--offer-file`, `--offer-stdin` is required.
- For `offer accept`, exactly one of `--offer-json`, `--offer-file`, `--offer-stdin` is required.
- For `offer submit-ord`, exactly one of `--psbt`, `--psbt-file`, `--psbt-stdin` is required.
- `offer create` requires `--ord-url` and inscription metadata available from ord indexer.
- `offer create --seller-payout-address` is optional; when omitted, payout defaults to the inscription output address from ord metadata.
- For dual-scheme sellers, pass `--seller-payout-address <payment-address>` to direct proceeds to the seller payment branch.
- `offer create --publisher-pubkey-hex` can override the default publisher pubkey embedded in the offer envelope.
- `offer publish` and `offer discover` require at least one `--relay`.
- `--thumb` and `--no-thumb` are boolean toggles.
- In human mode, thumbnails are enabled by default unless `--no-thumb` or `--no-images` is set.
- In `--agent` mode, thumbnails are disabled by default unless `--thumb` is explicitly set.

## 7) Profiles, Accounts, and Waits

Use named profile and custom data directory:

```bash
zinc-cli --agent --profile bot-a --data-dir /var/lib/zinc wallet info
```

Switch account:

```bash
zinc-cli --agent account use --index 1
```

Wait for confirmation:

```bash
zinc-cli --agent wait tx-confirmed --txid <txid> --timeout-secs 300
```

## 8) Safety Notes

- Prefer setting `ZINC_WALLET_PASSWORD` once for automation.
- Use `--password-env` only when you need a non-default env var name.
- Avoid `--password` in shared process environments.
- `wallet init` in human mode prints the new seed phrase once; in `--agent` mode mnemonic output is redacted unless `--reveal` is set.
- In `--agent` mode, consume stdout as machine data and treat stderr as diagnostics only.

## 9) Agent Flow Integration Test

Run the complete agentic wallet flow:

```bash
ZINC_CLI_LIVE_TESTS=1 cargo test --test agent_flow test_agent_wallet_workflow -- --nocapture
```

Run the live offer create -> accept integration test:

```bash
ZINC_CLI_LIVE_TESTS=1 cargo test --test offer_live test_offer_create_and_accept_live -- --nocapture
```

Sample run from a funded regtest environment:

```text
[1] Import wallet with seed phrase (dual scheme)
  ✓ Network: regtest, Scheme: dual
[2] Get wallet info
  ✓ Profile: default, Network: regtest, Scheme: dual
[3] Sync chain
  ✓ Chain synced
[4] Sync ordinals
  ✓ Ordinals synced: 3 inscriptions
[5] List inscriptions
  ✓ Inscriptions listed: 3
[6] Get account 0 taproot/payment addresses
  ✓ A0 taproot=bcrt1p...lts0 payment=bcrt1q...pee3
[7] Get taproot address at index 3
  ✓ Taproot[3]: bcrt1p...yk0g
[8] Get account 0 balance
  ✓ Account 0 total sats before transfer: 3118181 (spendable: 3117191)
[9] Account list
  ✓ Account 0 taproot=bcrt1p...lts0 payment=bcrt1q...pee3
[10] Switch to account 1
  ✓ Switched to account 1 taproot=bcrt1p...emmn payment=bcrt1q...36mn
[11] Verify new taproot after account switch
  ✓ Address changed: bcrt1p...lts0 -> bcrt1p...ydta
  ✓ Account 1 total sats before transfer: 0
[12] Switch back to account 0
  ✓ Switched back to account 0
[13] Create PSBT to transfer 1000 sats from account 0 -> account 1 payment
  ✓ PSBT created
[14] Analyze PSBT
  ✓ PSBT analyzed
[15] Sign + finalize PSBT
  ✓ PSBT signed
[16] Broadcast transaction
  ✓ Broadcast txid=eb7696...6017
[17] Verify account 0 sees transfer tx
  ✓ Account 0 total sats after transfer: 3117040 (before 3118181)
[18] Verify account 1 sees transfer tx + balance increase
  ✓ Account 1 total sats after transfer: 1000 (before 0)

✅ Wallet workflow test passed!
test test_agent_wallet_workflow ... ok
```

If account 0 does not have enough spendable balance, the transfer portion is skipped by design.

## 10) Offer Live Integration Tests (Ord + Nostr)

Run the ord submit/list live round-trip:

```bash
ZINC_CLI_LIVE_TESTS=1 \
cargo test --test offer_live test_offer_ord_submit_and_list_live -- --nocapture
```

Run the nostr publish/discover live round-trip using default relay:

```bash
ZINC_CLI_LIVE_TESTS=1 \
cargo test --test offer_live test_offer_nostr_publish_discover_live -- --nocapture
```

Use a custom nostr relay endpoint:

```bash
ZINC_CLI_LIVE_TESTS=1 \
ZINC_CLI_TEST_NOSTR_RELAY_URL=wss://nostr-regtest.exittheloop.com \
cargo test --test offer_live test_offer_nostr_publish_discover_live -- --nocapture
```

Notes:
- Tests are opt-in and skipped unless `ZINC_CLI_LIVE_TESTS=1` is set.
- Live infra defaults used by the suite:
  - Esplora: `https://regtest.exittheloop.com/api`
  - Ord: `https://ord-regtest.exittheloop.com`
  - Nostr relay: `wss://nostr-regtest.exittheloop.com`
