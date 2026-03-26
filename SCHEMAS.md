# zinc-cli JSON Schemas (v1.0)

All commands support `--agent` and emit exactly one JSON object to `stdout`.

For full contract details (typing, compatibility, and command policy), see `COMMAND_CONTRACT_V1.md`.
For task-oriented command examples (human + agent), see `USAGE.md`.

## Envelope

Success:

```json
{
  "ok": true,
  "schema_version": "1.0",
  "command": "wallet info",
  "...": "command-specific fields"
}
```

Error:

```json
{
  "ok": false,
  "schema_version": "1.0",
  "command": "wallet info",
  "error": {
    "type": "invalid|config|auth|network|insufficient_funds|policy|not_found|internal",
    "message": "human-readable",
    "exit_code": 1
  }
}
```

## Core Commands

- `wallet init`:
  - `profile`, `network`, `scheme`, `account_index`, `phrase`
  - `phrase` is hidden unless `--reveal` is set
  - `words` appears only when `--reveal` is set
- `wallet import`:
  - `profile`, `network`, `scheme`, `account_index`, `imported`
  - `phrase` appears only when `--reveal` is set
- `wallet info`:
  - `profile`, `version`, `network`, `scheme`, `account_index`, `esplora_url`, `ord_url`, `bitcoin_cli`, `bitcoin_cli_args`, `has_persistence`, `has_inscriptions`, `updated_at_unix`
- `sync chain`:
  - `events` (array of sync event strings)
- `sync ordinals`:
  - `inscriptions` (count)
- `balance`:
  - `total`, `spendable` (both with `immature`, `trusted_pending`, `untrusted_pending`, `confirmed`), `inscribed_sats`
- `tx list`:
  - `transactions` (array of tx objects from `zinc-core`)

## PSBT Commands

- `psbt create`:
  - `psbt` (base64)
- `psbt analyze`:
  - `analysis` (object from Ordinal Shield analyzer)
- `psbt sign`:
  - `psbt` (base64 signed PSBT)
- `psbt broadcast`:
  - `txid`

Input modes for `analyze|sign|broadcast`:

- `--psbt <base64>`
- `--psbt-file <path>`
- `--psbt-stdin` (reads raw base64 PSBT from stdin)

Exactly one PSBT input mode is required.

Optional output files:

- `psbt create --out-file <path>`
- `psbt sign --out-file <path>`

## Offer Commands

- `offer create`:
  - `inscription`
  - `ask_sats`
  - `fee_rate_sat_vb`
  - `seller_address`
  - `seller_outpoint`
  - `seller_pubkey_hex`
  - `expires_at_unix`
  - `thumbnail_lines` (optional)
  - `hide_inscription_ids`
  - `raw_response` (canonical full response payload)
- `offer publish`:
  - `event_id`
  - `accepted_relays` (count)
  - `total_relays` (count)
  - `publish_results` (per-relay acceptance/message rows)
  - `raw_response`
- `offer discover`:
  - `event_count`
  - `offer_count`
  - `offers`
  - `thumbnail_lines` (optional)
  - `hide_inscription_ids`
  - `raw_response`
- `offer submit-ord`:
  - `ord_url`
  - `submitted` (bool)
  - `raw_response`
- `offer list-ord`:
  - `ord_url`
  - `count`
  - `offers`
  - `raw_response`
- `offer accept`:
  - `inscription`
  - `ask_sats`
  - `txid`
  - `dry_run` (bool)
  - `inscription_risk`
  - `thumbnail_lines` (optional)
  - `hide_inscription_ids`
  - `raw_response`

Input modes and rules:

- For `offer publish`, exactly one of:
  - `--offer-json <json>`
  - `--offer-file <path>`
  - `--offer-stdin` (reads offer JSON from stdin)
- For `offer accept`, exactly one of:
  - `--offer-json <json>`
  - `--offer-file <path>`
  - `--offer-stdin` (reads offer JSON from stdin)
- For `offer submit-ord`, exactly one of:
  - `--psbt <base64>`
  - `--psbt-file <path>`
  - `--psbt-stdin` (reads base64 PSBT from stdin)
- `offer create` requires ord indexer metadata (`--ord-url`) for the target inscription.
- `offer create` accepts optional `--seller-payout-address` to override payout destination output.
- `offer create` accepts optional `--publisher-pubkey-hex` to override the embedded envelope publisher key.
- `offer publish` and `offer discover` require at least one `--relay`.

## Account/Wait/Snapshot

- `account list`: `accounts`
- `account use`: `previous_account_index`, `account_index`, `taproot_address`, `payment_address?`
- `wait tx-confirmed`: `txid`, `confirmation_time`, `confirmed`, `waited_secs`
- `wait balance`: `confirmed`, `confirmed_balance`, `target`, `waited_secs`
- `snapshot save`: `snapshot`
- `snapshot restore`: `restored`
- `snapshot list`: `snapshots`
- `lock info`: `profile`, `lock_path`, `locked`, `owner_pid`, `created_at_unix`, `age_secs`
- `lock clear`: `profile`, `lock_path`, `cleared`

## Scenario (Regtest)

- `scenario mine`: `blocks`, `address`, `raw_output`
- `scenario fund`: `address`, `amount_btc`, `txid`, `mine_blocks`, `mine_address`, `generated_blocks`
- `scenario reset`: `removed` (paths)

## Doctor

- `doctor`:
  - `healthy`
  - `esplora_url`
  - `esplora_reachable`
  - `ord_url`
  - `ord_reachable`
  - `ord_indexing_height`
  - `ord_error`
