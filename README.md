<p align="center">
  <img src="./assets/zinc-cli-logo.gif" alt="zinc-cli logo" width="260" />
</p>

# zinc-cli

`zinc-cli` is an agent-first Bitcoin + Ordinals wallet CLI.

It uses an account-based model where each account has:
- a taproot address for ordinals
- a native segwit address for BTC payments

Default behavior is optimized for automation (`--agent` JSON envelopes).
Optional interactive dashboard mode is available via `--features ui`.

## Install

Cargo (available now):

```bash
cargo install zinc-wallet-cli
```

Cargo with human dashboard/TUI:

```bash
cargo install zinc-wallet-cli --features ui
```

Homebrew (after tap is published):

```bash
brew tap bitcoiner-dev/zinc-cli
brew install zinc-cli
```

## Agent Quick Start (60 seconds)

```bash
export ZINC_WALLET_PASSWORD='your-wallet-password'
zinc-cli wallet init --overwrite
zinc-cli --agent wallet info
zinc-cli --agent sync chain
zinc-cli --agent sync ordinals
zinc-cli --agent balance
```

Expected agent envelope:

```json
{
  "ok": true,
  "schema_version": "1.0",
  "command": "wallet info"
}
```

## Human Mode

The `ui` feature enables a basic terminal dashboard for humans that shows:
- account balance
- inscriptions
- ordinals/payment addresses per account

Even without `ui`, human command output can be tuned with:
- `--thumb` to force thumbnail rendering on
- `--no-thumb` to disable thumbnail rendering

Run dashboard:

```bash
zinc-cli dashboard
```

## First 5 Minutes Demo

```bash
./demo/scripts/run_demo.sh
./demo/scripts/benchmark.sh --runs 5
```

Reports are written to `demo/artifacts/`.

## What You Can Do

- wallet lifecycle: `wallet init|import|info|reveal-mnemonic`
- sync: `sync chain|ordinals`
- addresses and balance: `address taproot|payment`, `balance`
- transfers: `psbt create|analyze|sign|broadcast`
- advanced offers (hidden from top-level help): `offer create|publish|discover|accept|submit-ord|list-ord`
- accounts: `account list|use`
- waits and tx: `wait tx-confirmed|balance`, `tx list`
- operations: `snapshot save|restore|list`, `lock info|clear`, `doctor`

## Docs

- Usage guide: [USAGE.md](./USAGE.md)
- Agent workflows: [AGENT_PLAYBOOKS.md](./AGENT_PLAYBOOKS.md)
- Command contract: [COMMAND_CONTRACT_V1.md](./COMMAND_CONTRACT_V1.md)
- JSON schemas: [SCHEMAS.md](./SCHEMAS.md)
- Demo kit: [demo/README.md](./demo/README.md)
- Reference clients: [examples/agent_client.py](./examples/agent_client.py), [examples/agent_client.ts](./examples/agent_client.ts)

## Security Notes

- Prefer `ZINC_WALLET_PASSWORD` (default password env) or `--password-stdin`.
- Use `--password-env` only when you need a non-default env var name.
- In `--agent` mode, mnemonic output is redacted unless `--reveal` is set.
- See [SECURITY.md](./SECURITY.md).

## License

MIT. See [LICENSE](./LICENSE).
