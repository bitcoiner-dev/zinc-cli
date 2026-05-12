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
  - `profile`, `network`, `scheme`, `payment_address_type`, `account_index`, `phrase`
  - `phrase` is hidden unless `--reveal` is set
  - `words` appears only when `--reveal` is set
- `wallet import`:
  - `profile`, `network`, `scheme`, `payment_address_type`, `account_index`, `mode`, `imported`
  - `phrase` appears only when `--reveal` is set
- `wallet info`:
  - `profile`, `version`, `network`, `scheme`, `payment_address_type`, `account_index`, `esplora_url`, `ord_url`, `bitcoin_cli`, `bitcoin_cli_args`, `has_persistence`, `has_inscriptions`, `updated_at_unix`
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

## Listing Commands

Fixed-price listings are seller-initiated sale PSBT envelopes. They are distinct from buyer-initiated `offer` envelopes.

- `listing create`: `inscription`, `ask_sats`, `fee_rate_sat_vb`, `seller_outpoint`, `passthrough_outpoint`, `seller_pubkey_hex`, `coordinator_pubkey_hex`, `expires_at_unix`, `raw_response`
- `listing sell`: raw JSON with `action`, `inscription`, `ask_sats`, `fee_rate_sat_vb`, `seller_outpoint`, `passthrough_outpoint`, `seller_pubkey_hex`, `coordinator_pubkey_hex`, `expires_at_unix`, `listing`, `activation?`, `publish?`
- `listing activate`: `inscription`, `txid`, `dry_run`, `inscription_risk`, `raw_response`
- `listing publish`: `event_id`, `accepted_relays`, `total_relays`, `publish_results`, `raw_response`
- `listing discover`: `event_count`, `listing_count`, `listings`, `raw_response`
- `listing buy`: `inscription`, `ask_sats`, `fee_sats`, `buyer_input_count`, `raw_response`
- `listing coordinator-sign`: `inscription`, `ask_sats`, `raw_response`
- `listing finalize`: `inscription`, `ask_sats`, `txid`, `broadcast`, `raw_response`
- `listing purchase`: raw JSON with `action`, `inscription`, `ask_sats`, `fee_sats`, `seller_input_index`, `buyer_input_count`, `buyer_receive_output_index`, `psbt`, `listing`, `coordinator?`, `finalized?`

Input modes and rules:

- For listing source commands, exactly one of `--listing-json`, `--listing-file`, `--listing-stdin` is required.
- `--password-stdin` cannot be combined with `--listing-stdin`.
- `listing publish` and `listing discover` require at least one `--relay`.
- `listing sell` is the high-level seller wrapper; `--relay` requires `--secret-key-hex`, and `--dry-run` requires `--activate`.
- `listing purchase` is the high-level buyer wrapper; it accepts either a listing source or relay discovery, and relay discovery requires `--expect-inscription`.
- `listing create` requires ord indexer metadata and the inscription must already be in the wallet.
- `listing activate` is the explicit TX1 signing/broadcast step; `--dry-run` signs locally without broadcast.

## Account/Wait/Snapshot

## Hosted Market

Hidden `insight market` commands return raw JSON from the Pulse ord.net gateway. The gateway owns upstream ord.net session storage; the CLI only uses normal Pulse auth.

- `pulse ordnet bind`: `bound`, `provider`, `ordinals_address`, `payment_address`, `requires_confirmed_payment_balance_btc`, `raw_response`
- `insight market listings|sales|collection-inscriptions|offers|my-offers`: raw upstream JSON
- `insight market buy-preflight`: raw upstream preflight JSON; CLI includes active payment public key and expectation metadata
- JSON-body market commands require exactly one of `--json`, `--file`, `--stdin`
- Submit-phase market commands require explicit expectation flags, validate those expectations against the preflight JSON, add signed PSBT fields, and return raw upstream submit JSON
- If Pulse is configured with a non-ord.net trading provider such as Satflow, hosted market commands fail with a `capability` error because Satflow is metadata/statistics only.

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
