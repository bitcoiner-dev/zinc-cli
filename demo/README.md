# Demo Kit: First 5 Minutes

This directory provides a reproducible evaluation flow for `zinc-cli` focused on agent usage.

The goal is to let evaluators run a deterministic workflow quickly and get machine-readable
artifacts for results and timing.

## What You Get

- `scripts/run_demo.sh`: one end-to-end agent workflow run with pass/fail output
- `scripts/benchmark.sh`: repeated runs with p50/p95/avg timing summary
- `env.sh`: default demo environment values
- `artifacts/`: generated JSON/Markdown reports

## Prerequisites

- `bash`
- `cargo`
- `jq`
- `python3` (used only for high-resolution timestamps in scripts)

## Quick Start

From repo root:

```bash
./demo/scripts/run_demo.sh
./demo/scripts/benchmark.sh --runs 5
```

Artifacts are written to `demo/artifacts/`.

## Defaults

By default the demo points to regtest endpoints:

- `ZINC_CLI_ESPLORA_URL=https://regtest.exittheloop.com/api`
- `ZINC_CLI_ORD_URL=https://ord-regtest.exittheloop.com`
- `ZINC_CLI_NETWORK=regtest`
- `ZINC_CLI_SCHEME=dual`

You can override any value by exporting env vars before running scripts.

## Evaluation Criteria

The first-5-minutes script validates:

1. wallet import/info
2. chain + ordinals sync
3. inscription listing
4. account/address operations
5. optional BTC transfer flow (only if spendable funds are sufficient)

The script marks the run as:

- `passed`: required flow succeeded
- `failed`: any required step failed

## Notes

- The transfer section is intentionally conditional. It is skipped when account 0 does not have
  enough spendable sats to send 1000 sats plus fee buffer.
- Scripts use `--agent` mode and produce JSON envelopes suitable for automation pipelines.
