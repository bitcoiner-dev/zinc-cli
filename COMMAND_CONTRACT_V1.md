# zinc-cli Command Contract v1

Status: Implemented (`schema_version = "1.0"`) as of 2026-02-26
Scope: Native CLI command contract for both human users and AI agents.

This document defines the active `v1` command contract. It is additive with current CLI JSON outputs so existing integrations can migrate safely.

## 1) Design Goals

1. One wallet engine and one command model for both humans and agents.
2. Stable machine-readable responses in `--agent` mode.
3. Strict, typed error taxonomy for automation reliability.
4. Backward-compatible rollout from current `SCHEMAS.md` behavior.

## 2) Execution Profiles

1. Human profile
- Default output mode (`ZINC_CLI_OUTPUT=human`).
- Output is a curated, styled, user-friendly presentation.

2. Agent profile
- CLI called with `--agent` or `ZINC_CLI_OUTPUT=agent`.
- Exactly one JSON object on `stdout` per invocation.
- Non-JSON noise must not be printed to `stdout`.

## 3) Global Flags (Supported)

`--agent`, `--yes`, `--password-env`, `--password-stdin`, `--reveal`, `--data-dir`, `--profile`, `--network`, `--scheme`, `--payment-address-type`, `--esplora-url`, `--ord-url`, `--pulse-url`, `--pulse-api-token`, `--ascii`, `--no-images`, `--thumb`, `--no-thumb`, `--correlation-id`, `--log-json`, `--idempotency-key`, `--network-timeout-secs`, `--network-retries`, `--policy-mode`

Global flags are supported both before and after command tokens.

Password defaults:
- If `--password-env` is omitted, the CLI reads password from `ZINC_WALLET_PASSWORD`.

Reliability defaults:
- `--network-timeout-secs` defaults to `30`.
- `--network-retries` defaults to `0`.
- `--policy-mode` defaults to `warn`.

Human-output defaults:
- Thumbnails are enabled by default in human mode.
- `--no-thumb` or `--no-images` disables thumbnails.
- In `--agent` mode thumbnails are disabled by default unless `--thumb` is set.

## 4) JSON Envelope

## Success

```json
{
  "ok": true,
  "schema_version": "1.0",
  "correlation_id": "zinc-...",
  "command": "wallet info",
  "meta": {
    "started_at_unix_ms": 1710000000000,
    "duration_ms": 42
  },
  "...": "command-specific fields"
}
```

## Error

```json
{
  "ok": false,
  "schema_version": "1.0",
  "correlation_id": "zinc-...",
  "command": "wallet info",
  "meta": {
    "started_at_unix_ms": 1710000000000,
    "duration_ms": 42
  },
  "error": {
    "type": "invalid|config|auth|network|insufficient_funds|policy|not_found|internal",
    "message": "human-readable",
    "exit_code": 1
  }
}
```

Notes:
1. `schema_version` and `command` are target-required for v1 implementation.
2. `correlation_id` is stable for one invocation and should be used by agents for tracing.
3. Clients must ignore unknown fields for forward compatibility.

Idempotency for mutating commands:
1. When `--idempotency-key <key>` is provided on a mutating command, successful results are cached.
2. Repeating the same mutating command payload with the same key replays the cached success result.
3. Reusing the same key for a different mutating payload returns `error.type=invalid`.
4. Mutating success payloads include additive field:
`idempotency: { key: string, replayed: bool, recorded_at_unix_ms: u128 }`

## 5) Exit Codes and Error Taxonomy

| error.type | exit_code | Meaning |
|---|---:|---|
| `invalid` | 2 | invalid command, flags, or values |
| `config` | 10 | local profile/config/storage issue |
| `auth` | 11 | password/decryption/auth issue |
| `network` | 12 | remote API/network/connectivity issue |
| `insufficient_funds` | 13 | balance insufficient for requested spend/fees |
| `policy` | 14 | ordinals/policy/security guard blocked operation |
| `not_found` | 15 | requested profile/resource/tx not found |
| `internal` | 1 | unexpected internal error |

## 6) Shared Type Definitions

| Type | Values / Shape |
|---|---|
| `Network` | `bitcoin \| signet \| testnet \| regtest` |
| `Scheme` | `unified \| dual` |
| `PaymentAddressType` | `native \| nested \| legacy` |
| `SatsBreakdown` | `{ immature: u64, trusted_pending: u64, untrusted_pending: u64, confirmed: u64 }` |
| `BalanceResponse` | `{ total: SatsBreakdown, spendable: SatsBreakdown, inscribed_sats: u64 }` |
| `TxItem` | `{ txid: string, amount_sats: i64, fee_sats: u64, confirmation_time: u64 \| null, tx_type: "send" \| "receive", inscriptions: InscriptionDetails[], parent_txids: string[], index: usize }` |
| `InscriptionDetails` | `{ id: string, number: i64, content_type: string \| null }` |
| `Account` | `{ index: u32, label: string, taprootAddress: string, taprootPublicKey: string, paymentAddress: string \| null, paymentPublicKey: string \| null }` |
| `ProfileMode` | `"seed" \| "watch" \| "watch_address"` |
| `CollectionResult` | `{ stats: { slug: string, floor_sats: u64, ... }, metadata: { name: string, description: string, image_url: string, ... } }` |

`u64` and `i64` are JSON numbers in v1.

## 7) Command Contracts

All commands below describe `--agent` response payloads.

## 7.1 wallet init

Command:
`wallet init [--words 12|24] [--network ...] [--scheme ...] [--payment-address-type native|nested|legacy] [--esplora-url <url>] [--ord-url <url>] [--overwrite]`

Success fields:
`profile`, `version`, `network`, `scheme`, `payment_address_type`, `account_index`, `esplora_url`, `ord_url`, `bitcoin_cli`, `bitcoin_cli_args`, `phrase`

Notes:
1. `phrase` is redacted by default and only shows the real mnemonic when `--reveal` is set.
2. `words` is emitted only when `--reveal` is set.

## 7.2 wallet import

Command:
`wallet import [--mnemonic <phrase>] [--taproot-xpub <xpub>] [--payment-xpub <xpub>] [--address <addr>] [--network ...] [--scheme ...] [--payment-address-type native|nested|legacy] [--esplora-url <url>] [--ord-url <url>] [--overwrite]`

Success fields:
`profile`, `network`, `scheme`, `payment_address_type`, `account_index`, `mode`, `imported`

Optional fields:
`phrase` (only when `--reveal` is set)

## 7.3 wallet info

Command:
`wallet info`

Success fields:
`profile`, `version`, `network`, `scheme`, `mode`, `payment_address_type`, `account_index`, `esplora_url`, `ord_url`, `bitcoin_cli`, `bitcoin_cli_args`, `account_gap_limit`, `address_scan_depth`, `has_persistence`, `has_inscriptions`, `updated_at_unix`

## 7.4 sync chain

Command:
`sync chain`

Success fields:
`events` as `string[]`

## 7.5 sync ordinals

Command:
`sync ordinals`

Success fields:
`inscriptions` as `u64` count

## 7.6 address taproot

Command:
`address taproot [--index N] [--new]`

Success fields:
`type`, `address`

Where `type = "taproot"`.

## 7.7 address payment

Command:
`address payment [--index N] [--new]`

Success fields:
`type`, `address`

Where `type = "payment"`.

## 7.8 balance

Command:
`balance`

Success fields:
`total`, `spendable`, `inscribed_sats`

## 7.9 tx list

Command:
`tx list [--limit N]`

Success fields:
`transactions` as `TxItem[]`

## 7.10 psbt create

Command:
`psbt create --to <addr> --amount-sats <n> --fee-rate <n> [--out-file <path>]`

Success fields:
`psbt`

## 7.11 psbt analyze

Command:
`psbt analyze [--psbt <base64> | --psbt-file <path> | --psbt-stdin]`

Success fields:
`analysis`

Expected analysis object:
`warning_level`, `inscriptions_burned`, `inscription_destinations`, `fee_sats`, `warnings`, `inputs`, `outputs`

Additive policy fields:
`safe_to_send`, `inscription_risk`, `policy_reasons`, `policy`

`policy` shape:
`{ safe_to_send: bool, inscription_risk: "none" | "low" | "medium" | "high" | "unknown", reasons: string[] }`

## 7.12 psbt sign

Command:
`psbt sign [--psbt <base64> | --psbt-file <path> | --psbt-stdin] [--sign-inputs 0,1] [--sighash N] [--finalize] [--out-file <path>]`

Success fields:
`psbt`

Additive policy fields:
`safe_to_send`, `inscription_risk`, `policy_reasons`, `analysis`

## 7.13 psbt broadcast

Command:
`psbt broadcast [--psbt <base64> | --psbt-file <path> | --psbt-stdin]`

Success fields:
`txid`

Additive policy fields:
`safe_to_send`, `inscription_risk`, `policy_reasons`, `analysis`

## 7.14 account list

Command:
`account list [--count N]`

Success fields:
`accounts` as `Account[]`

## 7.15 account use

Command:
`account use --index N`

Success fields:
`previous_account_index`, `account_index`, `taproot_address`, `payment_address`

## 7.16 wait tx-confirmed

Command:
`wait tx-confirmed --txid <id> [--timeout-secs N] [--poll-secs N]`

Success fields:
`txid`, `confirmation_time`

Additive fields:
`confirmed`, `waited_secs`

## 7.17 wait balance

Command:
`wait balance --confirmed-at-least <n> [--timeout-secs N] [--poll-secs N]`

Success fields:
`confirmed`

Additive fields:
`confirmed_balance`, `target`, `waited_secs`

## 7.18 snapshot save

Command:
`snapshot save --name <name> [--overwrite]`

Success fields:
`snapshot`

## 7.19 snapshot restore

Command:
`snapshot restore --name <name>`

Success fields:
`restored`

## 7.20 snapshot list

Command:
`snapshot list`

Success fields:
`snapshots` as `string[]`

## 7.21 lock info

Command:
`lock info`

Success fields:
`profile`, `lock_path`, `locked`, `owner_pid`, `created_at_unix`, `age_secs`

## 7.22 lock clear

Command:
`lock clear`

Success fields:
`profile`, `lock_path`, `cleared`

## 7.23 scenario mine

Command:
`scenario mine [--blocks N] [--address <addr>]`

Success fields:
`blocks`, `address`, `raw_output`

## 7.24 scenario fund

Command:
`scenario fund [--amount-btc <decimal>] [--address <addr>] [--mine-blocks N]`

Success fields:
`address`, `amount_btc`, `txid`, `mine_blocks`, `mine_address`, `generated_blocks`

## 7.25 scenario reset

Command:
`scenario reset [--remove-profile] [--remove-snapshots]`

Success fields:
`removed`

## 7.26 doctor

Command:
`doctor`

Success fields:
`healthy`, `esplora_url`, `esplora_reachable`, `ord_url`, `ord_reachable`, `ord_indexing_height`, `ord_error`

## 7.27 offer create

Command:
`offer create --inscription <id> --amount <u64> --fee-rate <u64> [--expires-in-secs <u64>] [--created-at-unix <unix>] [--nonce <u64>] [--seller-payout-address <addr>] [--publisher-pubkey-hex <xonly-hex>] [--submit-ord] [--offer-out-file <path>] [--psbt-out-file <path>]`

Notes:
- `--amount` has alias `--ask-sats`.
- `--ord-url` must be configured or provided as a global override.
- `--seller-payout-address` overrides payout destination output while preserving seller input metadata from ord inscription output.

Success fields:
`inscription`, `ask_sats`, `fee_rate_sat_vb`, `seller_address`, `seller_outpoint`, `seller_pubkey_hex`, `expires_at_unix`, `thumbnail_lines?`, `hide_inscription_ids`, `raw_response`

## 7.28 offer publish

Command:
`offer publish [--offer-json <json> | --offer-file <path> | --offer-stdin] --secret-key-hex <hex> --relay <url>... [--created-at-unix <unix>] [--timeout-ms N]`

Success fields:
`event_id`, `accepted_relays`, `total_relays`, `publish_results`, `raw_response`

## 7.29 offer discover

Command:
`offer discover --relay <url>... [--limit N] [--timeout-ms N]`

Success fields:
`event_count`, `offer_count`, `offers`, `thumbnail_lines?`, `hide_inscription_ids`, `raw_response`

## 7.30 offer submit-ord

Command:
`offer submit-ord [--psbt <base64> | --psbt-file <path> | --psbt-stdin]`

Success fields:
`ord_url`, `submitted`, `raw_response`

## 7.31 offer list-ord

Command:
`offer list-ord`

Success fields:
`ord_url`, `count`, `offers`, `raw_response`

## 7.32 offer accept

Command:
`offer accept [--offer-json <json> | --offer-file <path> | --offer-stdin] [--expect-inscription <id>] [--expect-ask-sats <u64>] [--dry-run]`

Success fields:
`inscription`, `ask_sats`, `txid`, `dry_run`, `inscription_risk`, `thumbnail_lines?`, `hide_inscription_ids`, `raw_response`

## 7.32.0 listing sell

Command:
`listing sell --inscription <id> --amount <u64> --fee-rate <u64> --coordinator-pubkey-hex <xonly-hex> [--expires-in-secs <u64>] [--created-at-unix <unix>] [--nonce <u64>] [--seller-payout-address <addr>] [--recovery-address <addr>] [--activate] [--dry-run] [--relay <url>... --secret-key-hex <hex>] [--listing-out-file <path>] [--tx1-out-file <path>] [--sale-psbt-out-file <path>] [--recovery-psbt-out-file <path>] [--signed-tx1-out-file <path>] [--timeout-ms N]`

Success fields:
raw JSON with `action`, `inscription`, `ask_sats`, `fee_rate_sat_vb`, `seller_outpoint`, `passthrough_outpoint`, `seller_pubkey_hex`, `coordinator_pubkey_hex`, `expires_at_unix`, `listing`, `ord_url`, `activation?`, `publish?`

Notes:
1. Agent convenience wrapper for seller flow.
2. Builds the same listing envelope and PSBTs as `listing create`.
3. When `--activate` is set, signs TX1; `--dry-run` avoids broadcast.
4. When `--relay` is supplied, `--secret-key-hex` is required and the listing is published after creation/activation.

## 7.32.1 listing create

Command:
`listing create --inscription <id> --amount <u64> --fee-rate <u64> --coordinator-pubkey-hex <xonly-hex> [--expires-in-secs <u64>] [--created-at-unix <unix>] [--nonce <u64>] [--seller-payout-address <addr>] [--recovery-address <addr>] [--listing-out-file <path>] [--tx1-out-file <path>] [--sale-psbt-out-file <path>] [--recovery-psbt-out-file <path>]`

Success fields:
`inscription`, `ask_sats`, `fee_rate_sat_vb`, `seller_outpoint`, `passthrough_outpoint`, `seller_pubkey_hex`, `coordinator_pubkey_hex`, `expires_at_unix`, `raw_response`

Notes:
1. Seller-initiated fixed-price listing flow; distinct from buyer-initiated `offer`.
2. Requires the inscription to already be in the wallet.
3. Builds TX1, sale, and recovery PSBTs; the sale PSBT is seller-signed with `SIGHASH_SINGLE|ANYONECANPAY`.

## 7.32.2 listing activate

Command:
`listing activate [--listing-json <json> | --listing-file <path> | --listing-stdin] [--dry-run] [--signed-tx1-out-file <path>]`

Success fields:
`inscription`, `txid`, `dry_run`, `inscription_risk`, `raw_response`

## 7.32.3 listing publish

Command:
`listing publish [--listing-json <json> | --listing-file <path> | --listing-stdin] --secret-key-hex <hex> --relay <url>... [--created-at-unix <unix>] [--timeout-ms N]`

Success fields:
`event_id`, `accepted_relays`, `total_relays`, `publish_results`, `raw_response`

## 7.32.4 listing discover

Command:
`listing discover --relay <url>... [--limit N] [--timeout-ms N]`

Success fields:
`event_count`, `listing_count`, `listings`, `raw_response`

## 7.32.5 listing buy

Command:
`listing buy [--listing-json <json> | --listing-file <path> | --listing-stdin] [--expect-inscription <id>] [--expect-ask-sats <u64>] [--listing-out-file <path>] [--psbt-out-file <path>]`

Success fields:
`inscription`, `ask_sats`, `fee_sats`, `buyer_input_count`, `raw_response`

## 7.32.6 listing coordinator-sign

Command:
`listing coordinator-sign [--listing-json <json> | --listing-file <path> | --listing-stdin] --secret-key-hex <hex> [--created-at-unix <unix>] [--listing-out-file <path>] [--psbt-out-file <path>]`

Success fields:
`inscription`, `ask_sats`, `raw_response`

## 7.32.7 listing finalize

Command:
`listing finalize [--listing-json <json> | --listing-file <path> | --listing-stdin] [--broadcast] [--finalized-psbt-out-file <path>] [--tx-hex-out-file <path>]`

Success fields:
`inscription`, `ask_sats`, `txid`, `broadcast`, `raw_response`

## 7.32.8 listing purchase

Command:
`listing purchase ([--listing-json <json> | --listing-file <path> | --listing-stdin] | --relay <url>...) [--expect-inscription <id>] [--expect-ask-sats <u64>] [--limit N] [--timeout-ms N] [--coordinator-secret-key-hex <hex>] [--finalize] [--broadcast] [--listing-out-file <path>] [--psbt-out-file <path>] [--finalized-psbt-out-file <path>] [--tx-hex-out-file <path>]`

Success fields:
raw JSON with `action`, `inscription`, `ask_sats`, `fee_sats`, `seller_input_index`, `buyer_input_count`, `buyer_receive_output_index`, `psbt`, `listing`, `coordinator?`, `finalized?`

Notes:
1. Agent convenience wrapper for buyer flow.
2. Accepts either a listing source or relay discovery, not both.
3. `--expect-inscription` is required when purchasing from relay discovery.
4. `--finalize` requires `--coordinator-secret-key-hex`; `--broadcast` requires `--finalize`.

## 7.33 pulse login

Command:
`pulse login [--token <token>] [--global] [--no-open]`

Success fields:
`message`

Notes:
1. Defaults to OAuth2 Device Authorization flow (URL + code).
2. Auto-opens browser unless `--no-open` is specified.
3. Use `--token <token>` for manual/CI login (legacy behavior).
4. Supports `--global` to persist session in global config; otherwise per-profile.

## 7.33.1 pulse ordnet bind

Command:
`pulse ordnet bind`

Success fields:
`bound`, `provider`, `ordinals_address`, `payment_address`, `requires_confirmed_payment_balance_btc`, `raw_response`

Notes:
1. Requires an authenticated Pulse session.
2. Requests ord.net challenges through Pulse, signs each challenge with the active wallet using BIP-322 simple hex signatures, and asks Pulse to verify/store the upstream ord.net session.
3. The CLI stores no ord.net API key or ord.net bearer token.
4. ord.net authenticated trading requires the bound payment address to satisfy the 0.01 BTC confirmed balance requirement.

## 7.33.2 pulse whoami

Command:
`pulse whoami [--global]`

Success fields:
`logged_in`, `sub`, `client_id`, `expires_at`, `scopes`, `scope`

Notes:
1. Displays current authentication status for Pulse services.

## 7.33.3 pulse logout

Command:
`pulse logout [--global]`

Success fields:
`message`

Notes:
1. Revokes the active session and clears local credentials.

## 7.33.4 insight market

Commands:
`insight market listings [--collection-slug <slug>] [--inscription-id <id>] [--seller-address <addr>] [--sort <recent|price>] [--limit N] [--cursor <cursor>]`
`insight market sales [--collection-slug <slug>] [--limit N] [--cursor <cursor>]`
`insight market collection-inscriptions --slug <slug> [--sort <oldest|newest>] [--limit N] [--cursor <cursor>]`
`insight market buy-preflight --collection-slug <slug> --listing-id <id> --inscription-id <id> [--expect-price-sats <sats>] [--raw-out-file <path>]`
`insight market buy-submit --collection-slug <slug> --expect-inscription <id> --expect-listing-id <id> --expect-price-sats <sats> [--expect-seller-address <addr>] [--expect-buyer-address <addr>] [--json <json> | --file <path> | --stdin]`
`insight market list-preflight --collection-slug <slug> [--json <json> | --file <path> | --stdin]`
`insight market list-submit --collection-slug <slug> --expect-inscription <id> --expect-price-sats <sats> [--expect-seller-address <addr>] [--json <json> | --file <path> | --stdin]`
`insight market delist --collection-slug <slug> --expect-inscription <id> --expect-listing-id <id> [--expect-seller-address <addr>] [--json <json> | --file <path> | --stdin]`
`insight market offers --inscription-id <id> [--history] [--page N]`
`insight market offer-create --collection-slug <slug> [--submit] [--expect-inscription <id>] [--expect-price-sats <sats>] [--expect-buyer-address <addr>] [--json <json> | --file <path> | --stdin]`
`insight market offer-cancel --inscription-id <id> --offer-id <id>`
`insight market offer-reject --inscription-id <id> --offer-id <id>`
`insight market offer-accept --inscription-id <id> --offer-id <id> [--submit] [--expect-price-sats <sats>] [--expect-seller-address <addr>] [--expect-buyer-address <addr>] [--json <json> | --file <path> | --stdin]`
`insight market offer-counter --inscription-id <id> --offer-id <id> [--submit] [--reject] [--accept] [--expect-price-sats <sats>] [--expect-seller-address <addr>] [--expect-buyer-address <addr>] [--json <json> | --file <path> | --stdin]`
`insight market my-offers [--view <owned|sent|history>] [--limit N] [--cursor <cursor>]`

Success fields:
raw JSON returned by the Pulse ord.net gateway.

Notes:
1. These commands are hidden, agent-facing hosted-market commands.
2. They require Pulse authentication and an ord.net wallet binding for upstream authenticated access.
3. JSON-body commands require exactly one of `--json`, `--file`, or `--stdin`.
4. `buy-preflight` injects the active wallet payment public key and sends expectations for agent auditability.
5. Submit-phase commands validate supplied `--expect-*` values against the preflight JSON before signing.
6. Submit-phase commands analyze each unsigned PSBT step, enforce `--policy-mode strict`, sign only upstream-declared input indices when present, and submit the signed payload through Pulse.
7. Hosted trading requires Pulse to be configured with the `ordnet` trading provider. Satflow-backed Pulse data is metadata/statistics only and returns `capability`/`trading_provider_unsupported` for hosted trading routes.

## 7.34 insight appraise

Command:
`insight appraise [--known-only]`

Success fields:
Array of `{ inscription_id: string, number: i64, collection: string | null, floor_sats: u64 | null }`

Notes:
1. Returns appraisal and collection data for all inscriptions in the current account.
2. Uses the configured Pulse Oracle.

## 7.35 insight search

Command:
`insight search <query>`

Success fields:
Array of `CollectionResult`

Notes:
1. Searches for collection metadata and floor prices matching the query string.

## 8) Input Source Rules...

For `psbt analyze`, `psbt sign`, `psbt broadcast`, and `offer submit-ord` exactly one PSBT input source must be present:

1. `--psbt <base64>`
2. `--psbt-file <path>`
3. `--psbt-stdin`

If zero or multiple are provided, return `invalid`.

`--password-stdin` must not be combined with `--psbt-stdin` in the same invocation.

For `offer publish` and `offer accept`, exactly one offer source must be present:

1. `--offer-json <json>`
2. `--offer-file <path>`
3. `--offer-stdin`

If zero or multiple are provided, return `invalid`.

For `offer publish` and `offer discover`, at least one `--relay <url>` is required.

`offer create` requires `--ord-url` and a resolvable inscription on that ord indexer.

For `listing activate`, `listing publish`, `listing buy`, `listing coordinator-sign`, and `listing finalize`, exactly one listing source must be present:

1. `--listing-json <json>`
2. `--listing-file <path>`
3. `--listing-stdin`

If zero or multiple are provided, return `invalid`.

For `listing publish` and `listing discover`, at least one `--relay <url>` is required.

For `listing purchase`, provide either exactly one listing source or at least one `--relay <url>`, not both. Relay discovery requires `--expect-inscription`.

`listing create` and `listing sell` require `--ord-url` and a resolvable wallet-owned inscription on that ord indexer.

When `--policy-mode strict` is set, `psbt sign`, `psbt broadcast`, `offer accept`, and `listing activate` fail closed with `error.type="policy"` for unsafe, medium/high, or unknown inscription-risk outcomes.

## 9) Compatibility and Versioning Policy

1. v1 changes must be additive by default.
2. Existing field names must not be renamed in v1.
3. Any planned removal requires:
- deprecation notice in docs
- one minor cycle with compatibility output
- explicit migration note

## 10) Security Contract for Agent Usage

1. Agents should always use `--agent`.
2. Agents must use password env variables or stdin; the insecure plaintext --password flag has been removed.
3. Policy/ordinals failures must be surfaced as `error.type = "policy"` with actionable messages.
4. Commands that mutate wallet state should remain explicit and single-purpose.
5. Agents should set `--idempotency-key` on mutating commands and tune `--network-timeout-secs`/`--network-retries` for reliability.

## 11) Structured Stderr Logs (Optional)

When `--log-json` is provided, the CLI emits JSON lines to `stderr` with:
- `event`: `command_start` \| `command_finish` \| `command_error`
- `correlation_id`
- `command`
- `ts_unix_ms`

These logs are additive and do not affect `stdout` contract shape.

## 12) Conformance Checklist

1. Every JSON response includes `ok`.
2. Every error response includes `error.type`, `error.message`, `error.exit_code`.
3. Each command returns only documented required fields plus optional additive fields.
4. Unknown command/flag/value paths map to `invalid` with exit code `2`.
5. Machine contract tests enforce envelope and representative command/error shape invariants.
