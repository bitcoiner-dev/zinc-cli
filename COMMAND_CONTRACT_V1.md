# zinc-cli Command Contract v1

Status: Implemented (`schema_version = "1.0"`) as of 2026-02-26
Scope: Native CLI command contract for both human users and AI agents.

This document defines the active `v1` command contract. It is additive with current CLI JSON outputs so existing integrations can migrate safely.

## 1) Design Goals

1. One wallet engine and one command model for both humans and agents.
2. Stable machine-readable responses in `--json` mode.
3. Strict, typed error taxonomy for automation reliability.
4. Backward-compatible rollout from current `SCHEMAS.md` behavior.

## 2) Execution Profiles

1. Human profile
- CLI called without `--json`.
- Output may be user-friendly text.

2. Agent profile
- CLI called with `--agent` (preferred) or `--json`.
- `--agent` implies `--json --quiet --ascii`.
- Exactly one JSON object on `stdout` per invocation.
- Non-JSON noise must not be printed to `stdout`.

## 3) Global Flags (Supported)

`--json`, `--agent`, `--quiet`, `--yes`, `--password`, `--password-env`, `--password-stdin`, `--reveal`, `--data-dir`, `--profile`, `--network`, `--scheme`, `--esplora-url`, `--ord-url`, `--ascii`, `--no-images`, `--correlation-id`, `--log-json`, `--idempotency-key`, `--network-timeout-secs`, `--network-retries`, `--policy-mode`

Global flags are supported both before and after command tokens.

Password defaults:
- If `--password-env` is omitted, the CLI reads password from `ZINC_WALLET_PASSWORD`.

Reliability defaults:
- `--network-timeout-secs` defaults to `30`.
- `--network-retries` defaults to `0`.
- `--policy-mode` defaults to `warn`.

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
| `SatsBreakdown` | `{ immature: u64, trusted_pending: u64, untrusted_pending: u64, confirmed: u64 }` |
| `BalanceResponse` | `{ total: SatsBreakdown, spendable: SatsBreakdown, inscribed_sats: u64 }` |
| `TxItem` | `{ txid: string, amount_sats: i64, fee_sats: u64, confirmation_time: u64 \| null, tx_type: "send" \| "receive", inscriptions: InscriptionDetails[], parent_txids: string[], index: usize }` |
| `InscriptionDetails` | `{ id: string, number: i64, content_type: string \| null }` |
| `Account` | `{ index: u32, label: string, taprootAddress: string, taprootPublicKey: string, paymentAddress: string \| null, paymentPublicKey: string \| null }` |

`u64` and `i64` are JSON numbers in v1.

## 7) Command Contracts

All commands below describe `--json` response payloads.

## 7.1 wallet init

Command:
`wallet init [--words 12|24] [--network ...] [--scheme ...] [--esplora-url <url>] [--ord-url <url>] [--overwrite]`

Success fields:
`profile`, `version`, `network`, `scheme`, `account_index`, `esplora_url`, `ord_url`, `bitcoin_cli`, `bitcoin_cli_args`, `phrase`

Notes:
1. `phrase` is redacted by default and only shows the real mnemonic when `--reveal` is set.
2. `words` is emitted only when `--reveal` is set.

## 7.2 wallet import

Command:
`wallet import --mnemonic <phrase> [--network ...] [--scheme ...] [--esplora-url <url>] [--ord-url <url>] [--overwrite]`

Success fields:
`profile`, `network`, `scheme`, `account_index`, `imported`

Optional fields:
`phrase` (only when `--reveal` is set)

## 7.3 wallet info

Command:
`wallet info`

Success fields:
`profile`, `version`, `network`, `scheme`, `account_index`, `esplora_url`, `ord_url`, `bitcoin_cli`, `bitcoin_cli_args`, `has_persistence`, `has_inscriptions`, `updated_at_unix`

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
`action`, `blocks`, `address`, `raw`

## 7.24 scenario fund

Command:
`scenario fund [--amount-btc <decimal>] [--address <addr>] [--mine-blocks N]`

Success fields:
`action`, `address`, `amount_btc`, `txid`, `mine_blocks`, `mine_address`, `generated_blocks`

## 7.25 scenario reset

Command:
`scenario reset [--remove-profile] [--remove-snapshots]`

Success fields:
`action`, `removed`

## 7.26 doctor

Command:
`doctor`

Success fields:
`healthy`, `esplora`, `ord`

`esplora` shape:
`{ url: string, reachable: bool }`

`ord` shape:
`{ url: string, reachable: bool, indexing_height: u32 | null, error: string | null }`

## 7.27 offer create

Command:
`offer create --inscription <id> --amount <u64> --fee-rate <u64> [--expires-in-secs <u64>] [--created-at-unix <unix>] [--nonce <u64>] [--seller-payout-address <addr>] [--publisher-pubkey-hex <xonly-hex>] [--submit-ord] [--offer-out-file <path>] [--psbt-out-file <path>]`

Notes:
- `--amount` has alias `--ask-sats`.
- `--ord-url` must be configured or provided as a global override.
- `--seller-payout-address` overrides payout destination output while preserving seller input metadata from ord inscription output.

Success fields:
`inscription`, `seller_address`, `seller_outpoint`, `postage_sats`, `ask_sats`, `fee_rate_sat_vb`, `seller_input_index`, `buyer_input_count`, `psbt`, `offer`, `submitted_ord`, `ord_url`

## 7.28 offer publish

Command:
`offer publish [--offer-json <json> | --offer-file <path> | --offer-stdin] --secret-key-hex <hex> --relay <url>... [--created-at-unix <unix>] [--timeout-ms N]`

Success fields:
`event`, `publish_results`, `accepted_relays`, `total_relays`

## 7.29 offer discover

Command:
`offer discover --relay <url>... [--limit N] [--timeout-ms N]`

Success fields:
`events`, `offers`, `event_count`, `offer_count`

## 7.30 offer submit-ord

Command:
`offer submit-ord [--psbt <base64> | --psbt-file <path> | --psbt-stdin]`

Success fields:
`submitted`, `ord_url`

## 7.31 offer list-ord

Command:
`offer list-ord`

Success fields:
`ord_url`, `offers`, `count`

## 7.32 offer accept

Command:
`offer accept [--offer-json <json> | --offer-file <path> | --offer-stdin] [--expect-inscription <id>] [--expect-ask-sats <u64>] [--dry-run]`

Success fields:
`accepted`, `dry_run`, `offer_id`, `seller_input_index`, `input_count`, `inscription_id`, `ask_sats`, `safe_to_send`, `inscription_risk`, `policy_reasons`, `analysis`, `txid?`

## 8) Input Source Rules (PSBT and Offer Commands)

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

When `--policy-mode strict` is set, `psbt sign`, `psbt broadcast`, and `offer accept` fail closed with `error.type="policy"` for unsafe, medium/high, or unknown inscription-risk outcomes.

## 9) Compatibility and Versioning Policy

1. v1 changes must be additive by default.
2. Existing field names must not be renamed in v1.
3. Any planned removal requires:
- deprecation notice in docs
- one minor cycle with compatibility output
- explicit migration note

## 10) Security Contract for Agent Usage

1. Agents should always use `--json`.
2. Agents should prefer password env variables over plaintext flags.
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
