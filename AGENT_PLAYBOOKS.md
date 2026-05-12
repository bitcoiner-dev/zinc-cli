# Agent Playbooks

These playbooks are reference workflows for AI agents using `zinc-cli` in `--agent` mode.

Set password once per shell/session:

```bash
export ZINC_WALLET_PASSWORD='your-wallet-password'
```

## 1) Portfolio Snapshot (Read-Only)

Goal: Collect wallet posture for one account.

```bash
zinc-cli --agent wallet info
zinc-cli --agent sync chain
zinc-cli --agent sync ordinals
zinc-cli --agent balance
zinc-cli --agent inscription list
```

Success criteria:
- every response has `ok=true`
- `balance` is present
- `inscription list` returns an `inscriptions` array

## 2) Safe BTC Send Using PSBT Policy

Goal: Send BTC while programmatically checking ordinal risk.

```bash
zinc-cli --agent psbt create \
  --to <destination> --amount-sats <amount> --fee-rate <fee_rate>
zinc-cli --agent psbt analyze --psbt <PSBT>
zinc-cli --agent psbt sign --psbt <PSBT> --finalize
zinc-cli --agent psbt broadcast --psbt <SIGNED_PSBT>
```

Policy gate:
- require `safe_to_send=true`
- reject or escalate when `inscription_risk` is `medium`, `high`, or `unknown`
- include `policy_reasons` in agent reasoning/logging

## 3) Fund Account 1 from Account 0 (Dual Scheme)

Goal: Move a small BTC amount between accounts without inscription transfer.

```bash
zinc-cli --agent account use --index 1
zinc-cli --agent address payment
zinc-cli --agent account use --index 0
zinc-cli --agent psbt create \
  --to <account1_payment_address> --amount-sats 1000 --fee-rate 1
```

Verification:
- txid appears in `tx list` for both accounts after sync
- account 1 balance increases by at least the send amount
- no policy warnings indicating inscription burn risk

## 4) Optional Reliability/Safety Controls

Use these when needed; they are not required on every command.

Idempotency example (non-broadcast):

```bash
zinc-cli --agent --idempotency-key proposal-20260321-001 psbt create \
  --to <destination> --amount-sats <amount> --fee-rate <fee_rate>
```

Why here: if an agent retries PSBT creation after a timeout, this key replays the same successful result instead of generating a second, potentially different proposal.

Policy mode example:

```bash
zinc-cli --agent --policy-mode strict psbt sign --psbt <PSBT> --finalize
```

In `strict`, risky/unknown policy outcomes are blocked with `error.type=policy`. Default mode is `warn`.

## Correlation IDs and Structured Logs

For multi-step workflows, keep one correlation ID across commands:

```bash
CID="agent-run-2026-03-21-001"
zinc-cli --agent --log-json --correlation-id "$CID" ... 2>>workflow.log
```

The command envelope includes `correlation_id` and `meta.duration_ms`.
When `--log-json` is enabled, stderr emits structured lifecycle events (`command_start`, `command_finish`, `command_error`) that can be joined by `correlation_id`.

For unstable networks, add:

```bash
--network-timeout-secs 20 --network-retries 2
```

## 5) Fixed-Price Listing Purchase

Goal: Complete a seller-initiated fixed-price listing discovered over Nostr.

Recommended compact flow:

```bash
zinc-cli --agent listing purchase \
  --relay <relay-url> \
  --expect-inscription <inscription-id> \
  --expect-ask-sats <sats> \
  --listing-out-file /tmp/listing.buyer.json
```

Primitive checkpoint flow:

```bash
zinc-cli --agent listing discover --relay <relay-url> --limit 50
zinc-cli --agent listing buy \
  --listing-json '<listing-json>' \
  --expect-inscription <inscription-id> \
  --expect-ask-sats <sats> \
  --listing-out-file /tmp/listing.buyer.json
zinc-cli --agent listing coordinator-sign \
  --listing-file /tmp/listing.buyer.json \
  --secret-key-hex <coordinator-secret-key-hex> \
  --listing-out-file /tmp/listing.coordinator.json
zinc-cli --agent listing finalize \
  --listing-file /tmp/listing.coordinator.json \
  --broadcast
```

Agent checks:
- Treat `offer` and `listing` as different protocols: `offer` is buyer-initiated, `listing` is seller-initiated.
- Require expectation flags before buying a listing from a relay.
- Keep the updated listing envelope from each step; the sale PSBT is replaced as signatures are added.
- Prefer `listing purchase` for ordinary agent buys; use primitive commands when a workflow needs explicit resume points.

## 6) Hosted ord.net Market Discovery

Goal: Use Zinc/Pulse as the paid service boundary while ord.net acts as an upstream trading provider.

```bash
zinc-cli --agent pulse ordnet bind
zinc-cli --agent insight market listings --collection-slug <slug> --limit 20
zinc-cli --agent insight market buy-preflight \
  --collection-slug <slug> \
  --listing-id <listing-id> \
  --inscription-id <inscription-id> \
  --expect-price-sats <sats> \
  --raw-out-file /tmp/ordnet-buy-preflight.json

zinc-cli --agent insight market buy-submit \
  --collection-slug <slug> \
  --expect-inscription <inscription-id> \
  --expect-listing-id <listing-id> \
  --expect-price-sats <sats> \
  --file /tmp/ordnet-buy-preflight.json
```

Agent checks:
- Treat hosted ord.net market commands as separate from decentralized `offer` and `listing` commands.
- Bind with the active user wallet; ord.net requires the payment address to satisfy the 0.01 BTC confirmed balance requirement.
- Keep hosted writes two-phase: inspect preflight PSBT steps before any submit payload is sent.
- Provide exact `--expect-*` values for submit commands; the CLI rejects mismatches before signing.
- Do not route hosted trading through Satflow. Satflow-backed Pulse data is for metadata/statistics only.
