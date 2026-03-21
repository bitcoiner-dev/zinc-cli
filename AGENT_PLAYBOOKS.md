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
