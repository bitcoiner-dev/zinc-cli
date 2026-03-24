# zinc-cli JSON Schemas (v1.0)

All commands support `--json` and emit exactly one JSON object to `stdout`.

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

- `offer publish`:
  - `event` (signed nostr event)
  - `publish_results` (per-relay acceptance/message rows)
  - `accepted_relays` (count)
  - `total_relays` (count)
- `offer discover`:
  - `events` (decoded nostr events)
  - `offers` (decoded offer envelopes with event metadata)
  - `event_count`
  - `offer_count`
- `offer submit-ord`:
  - `submitted` (bool)
  - `ord_url`
- `offer list-ord`:
  - `ord_url`
  - `offers` (array of base64 PSBT strings)
  - `count`
- `offer accept`:
  - `accepted` (bool)
  - `dry_run` (bool)
  - `offer_id`
  - `seller_input_index`
  - `input_count`
  - `inscription_id`
  - `ask_sats`
  - `safe_to_send`
  - `inscription_risk`
  - `policy_reasons`
  - `analysis`
  - `txid` (present when `dry_run=false`)

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
- `offer publish` and `offer discover` require at least one `--relay`.

## Account/Wait/Snapshot

- `account list`: `accounts`
- `account use`: `account_index`
- `wait tx-confirmed`: `txid`, `confirmation_time`
- `wait balance`: `confirmed`
- `snapshot save`: `snapshot`
- `snapshot restore`: `restored`
- `snapshot list`: `snapshots`
- `lock info`: `profile`, `lock_path`, `locked`, `owner_pid`, `created_at_unix`, `age_secs`
- `lock clear`: `profile`, `lock_path`, `cleared`

## Scenario (Regtest)

- `scenario mine`: `action`, `blocks`, `address`, `raw`
- `scenario fund`: `action`, `address`, `amount_btc`, `txid`, `mine_blocks`, `mine_address`, `generated_blocks`
- `scenario reset`: `action`, `removed` (paths)

## Doctor

- `doctor`:
  - `healthy`
  - `esplora` (`url`, `reachable`)
  - `ord` (`url`, `reachable`, `indexing_height`, `error`)
