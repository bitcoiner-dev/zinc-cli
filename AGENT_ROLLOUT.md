# Zinc CLI Agent-First Improvements — Phased Rollout

*Inspired by [agcli](https://github.com/matthiasdebernardini/agcli) and "Designing Interfaces for AI Agents Instead of Humans"*

## Guiding Principles for Ordering

1. **Additive before breaking** — features that don't change existing output shapes ship first
2. **Foundation before abstraction** — fix error system before adding HATEOAS (HATEOAS depends on clean errors)
3. **Agent pain before developer pain** — what wastes the most agent tokens/reasoning first
4. **Low risk before high risk** — changes that touch output.rs before changes that touch main.rs dispatch

---

## Phase 1: Error System Overhaul (Foundation)

**Why first**: HATEOAS, next_actions, and streaming all need clean error types. The current substring-matching `map_wallet_error` is the root problem. Fix this before building on top of it.

| # | Change | Files | Effort |
|---|--------|-------|--------|
| 1.1 | Add `retryable: bool` to `AppError` variants | `error.rs` | Small |
| 1.2 | Add `fix: &'static str` to each `AppError` variant | `error.rs` | Small |
| 1.3 | Replace `map_wallet_error` substring matching with typed error codes from zinc-core | `wallet_service.rs`, `error.rs` | Medium |
| 1.4 | Add `code: String` field to error envelope (e.g., `WRONG_PASSWORD`, `ESPLORA_UNREACHABLE`, `INSUFFICIENT_FUNDS`) | `output.rs`, `main.rs` | Small |
| 1.5 | Include `fix` and `retryable` in the JSON error envelope | `output.rs`, `main.rs` | Small |
| 1.6 | Add `timestamp` to all envelopes (success + error) | `output.rs`, `main.rs` | Small |

**Target error envelope shape** (backward compatible — adding fields):
```json
{
  "ok": false,
  "schema_version": "1.0",
  "command": "psbt broadcast --psbt ...",
  "timestamp": 1740000000,
  "correlation_id": "...",
  "meta": { "started_at_unix_ms": ..., "duration_ms": ... },
  "error": {
    "exit_code": 12,
    "type": "network",
    "code": "ESPLORA_UNREACHABLE",
    "message": "esplora request failed: connection refused",
    "retryable": true,
    "fix": "Run 'zinc-cli doctor' to diagnose. Or set a working Esplora URL: zinc-cli config set esplora_url https://mempool.space/api"
  }
}
```

---

## Phase 2: Self-Documenting Command Tree

**Why second**: Agents need to discover commands before they can follow next_actions. This is the "page zero" problem.

| # | Change | Files | Effort |
|---|--------|-------|--------|
| 2.1 | In agent mode, when no subcommand is given, return JSON command tree instead of clap help text | `main.rs` | Medium |
| 2.2 | Add descriptions to all commands and subcommands (some are missing) | `cli.rs` | Small |
| 2.3 | Include `doctor` health summary in the root tree response (agent mode) | `main.rs`, `commands/doctor.rs` | Small |
| 2.4 | Expose hidden commands (`intent`, `pair`) as `advanced` section in agent mode tree | `main.rs` | Small |

**Target root response shape** (agent mode, no subcommand):
```json
{
  "ok": true,
  "command": "zinc-cli",
  "timestamp": 1740000000,
  "schema_version": "1.0",
  "correlation_id": "...",
  "meta": { "started_at_unix_ms": ..., "duration_ms": ... },
  "result": {
    "name": "zinc-cli",
    "version": "0.3.0",
    "description": "Agent-first Bitcoin + Ordinals CLI wallet",
    "commands": [
      { "name": "wallet", "description": "Wallet lifecycle (init, import, info, reveal-mnemonic)", "usage": "zinc-cli wallet <subcommand>" },
      { "name": "balance", "description": "Show wallet balance (confirmed, pending, inscribed sats)", "usage": "zinc-cli balance" },
      { "name": "psbt", "description": "PSBT operations (create, analyze, sign, broadcast)", "usage": "zinc-cli psbt <subcommand>" },
      { "name": "sync", "description": "Synchronize chain or ordinals data", "usage": "zinc-cli sync chain|ordinals" },
      { "name": "address", "description": "Show or generate addresses", "usage": "zinc-cli address taproot|payment [--new]" },
      { "name": "tx", "description": "Transaction history", "usage": "zinc-cli tx list [--limit]" },
      { "name": "inscription", "description": "Inscription management", "usage": "zinc-cli inscription list" },
      { "name": "account", "description": "Account management", "usage": "zinc-cli account list|use" },
      { "name": "wait", "description": "Wait for conditions (tx-confirmed, balance)", "usage": "zinc-cli wait <subcommand>" },
      { "name": "snapshot", "description": "Wallet state snapshots", "usage": "zinc-cli snapshot save|restore|list" },
      { "name": "doctor", "description": "Connectivity and health checks", "usage": "zinc-cli doctor" },
      { "name": "config", "description": "Configuration management", "usage": "zinc-cli config show|set|unset" },
      { "name": "setup", "description": "Interactive first-run setup", "usage": "zinc-cli setup" },
      { "name": "lock", "description": "Profile lock management", "usage": "zinc-cli lock info|clear" },
      { "name": "version", "description": "Show version", "usage": "zinc-cli version" }
    ],
    "health": {
      "esplora_reachable": true,
      "ord_reachable": true
    }
  },
  "next_actions": [
    { "command": "zinc-cli wallet init", "description": "Create a new wallet" },
    { "command": "zinc-cli doctor", "description": "Full connectivity check" }
  ]
}
```

---

## Phase 3: HATEOAS — `next_actions` on Every Response

**Why third**: Depends on Phase 1 (clean errors) and Phase 2 (command tree). This is the highest-impact agent UX change.

### 3.1 — Core Infrastructure

| # | Change | Files | Effort |
|---|--------|-------|--------|
| 3.1.1 | Add `NextAction` struct (`command: String`, `description: String`, `params: Option<HashMap<String, ActionParam>>`) | New: `next_action.rs` or inline in `output.rs` | Small |
| 3.1.2 | Add `next_actions: Vec<NextAction>` to success envelope | `output.rs` | Small |
| 3.1.3 | Add `next_actions: Vec<NextAction>` to error envelope | `output.rs` | Small |
| 3.1.4 | Define `ActionParam` struct (`value: Option<serde_json::Value>`, `description: Option<String>`, `required: Option<bool>`, `default: Option<serde_json::Value>`, `enum_values: Option<Vec<String>>`) | `next_action.rs` | Small |

### 3.2 — Command-Level next_actions

Populate contextual `next_actions` for each command:

| Command | next_actions |
|---------|-------------|
| `wallet init` | `balance`, `address taproot`, `sync chain` |
| `wallet import` | `balance`, `sync chain`, `sync ordinals` |
| `wallet info` | `balance`, `address taproot`, `account list` |
| `psbt create` | `psbt analyze` (always first), `psbt sign` (skip if risky) |
| `psbt analyze` | `psbt sign` (if safe), `psbt create` (if risky — guide to fix) |
| `psbt sign` | `psbt broadcast` |
| `psbt broadcast` | `wait tx-confirmed`, `balance` |
| `sync chain` | `balance`, `tx list`, `inscription list` |
| `sync ordinals` | `inscription list`, `balance` |
| `balance` | `address taproot`, `psbt create` |
| `tx list` | `balance`, `wallet info` |
| `inscription list` | `balance`, `address taproot` |
| `address taproot` | `balance`, `psbt create` |
| `address payment` | `balance`, `psbt create` |
| `wait tx-confirmed` | `balance`, `tx list` |
| `wait balance` | `balance` |
| `snapshot save` | `snapshot list` |
| `snapshot restore` | `balance`, `wallet info` |
| `account use` | `balance`, `address taproot` |
| `account list` | `account use --index <n>` |
| `config set` | `config show`, `doctor` |
| `doctor` | `wallet init` (if no wallet), `balance` (if healthy) |
| `lock clear` | `wallet info` |
| `scenario mine/fund` | `balance` |

### 3.3 — Error-Level next_actions

| Error Type | next_actions |
|------------|-------------|
| `auth` | `zinc-cli --help` (check password flags) |
| `network` | `doctor`, retry same command |
| `insufficient_funds` | `balance` |
| `not_found` | `wallet init` or `wallet import` |
| `policy` | `psbt analyze --psbt <psbt>` (re-run analysis) |
| `invalid` | `zinc-cli <command> --help` |

### 3.4 — Pre-filled Params

Some next_actions should pre-fill values from the current operation:

```json
{
  "command": "zinc-cli psbt broadcast --psbt <psbt>",
  "description": "Broadcast the signed transaction",
  "params": {
    "psbt": {
      "value": "cHNidP8BAHECA...",
      "description": "PSBT from psbt sign"
    }
  }
}
```

---

## Phase 4: Context Protection — Truncation with File Pointers

**Why fourth**: Agents hitting large wallets are blowing context windows. This is a concrete, measurable problem.

| # | Change | Files | Effort |
|---|--------|-------|--------|
| 4.1 | Add `truncate_json_output` utility: takes JSON value + byte limit → writes full to temp file, returns truncated + metadata | New: `truncate.rs` | Medium |
| 4.2 | Apply truncation to `tx list` (default: 50 items) | `commands/tx.rs` | Small |
| 4.3 | Apply truncation to `inscription list` (default: 50 items) | `commands/inscription.rs` | Small |
| 4.4 | Apply truncation to `config show` (hide default-null values) | `commands/config.rs` | Small |
| 4.5 | Apply truncation to `account list` (default: 20) | `commands/account.rs` | Small |
| 4.6 | Apply truncation to `snapshot list` (default: 20) | `commands/snapshot.rs` | Small |
| 4.7 | Add `--output-limit <n>` global flag to override default truncation limit | `cli.rs` | Small |

**Truncated output shape**:
```json
{
  "ok": true,
  "command": "tx list",
  "result": {
    "transactions": [...],
    "total": 847,
    "showing": 50,
    "truncated": true,
    "full_output": "/tmp/zinc-cli-tx-list-a1b2c3.json"
  },
  "next_actions": [...]
}
```

---

## Phase 5: NDJSON Streaming for `wait` and `sync`

**Why fifth**: Temporal operations are the most frustrating for agents — black-box hangs with no feedback. Ships behind `--stream` flag for backward compatibility.

| # | Change | Files | Effort |
|---|--------|-------|--------|
| 5.1 | Add `--stream` global flag that enables NDJSON output for streaming commands | `cli.rs` | Small |
| 5.2 | Define `StreamEvent` enum: `Start`, `Progress`, `Result`, `Error` | New: `stream.rs` | Medium |
| 5.3 | Implement NDJSON for `wait tx-confirmed`: emit `Start`, `Progress` every poll, then `Result`/`Error` | `commands/wait.rs` | Medium |
| 5.4 | Implement NDJSON for `wait balance`: same pattern | `commands/wait.rs` | Small |
| 5.5 | Implement NDJSON for `sync chain` and `sync ordinals`: `Start`, `Progress` per phase, `Result`/`Error` | `commands/sync.rs` | Medium |
| 5.6 | Ensure terminal `Result`/`Error` events match standard envelope shape (non-streaming consumers read last line) | `stream.rs` | Small |
| 5.7 | Tune `--poll-secs` defaults: 10s for `wait`, 5s for `sync` | `commands/wait.rs`, `commands/sync.rs` | Small |

**Without `--stream`**: current behavior (single JSON at end). **With `--stream`**:
```
{"type":"start","command":"wait tx-confirmed --txid abc123","timestamp":1740000000}
{"type":"progress","status":"waiting","txid":"abc123","elapsed_secs":30,"confirmed":false}
{"type":"progress","status":"confirmed","txid":"abc123","elapsed_secs":87,"confirmed":true}
{"type":"result","ok":true,"command":"wait tx-confirmed --txid abc123","timestamp":1740000087,"result":{"txid":"abc123","confirmed":true,"waited_secs":87},"next_actions":[...]}
```

---

## Phase 6: Output Format Separation

**Why last**: This is a DX refactor, not a new capability. Cleans up the conflation of `--agent`.

| # | Change | Files | Effort |
|---|--------|-------|--------|
| 6.1 | Add `--output json\|human` flag (default: `human`) | `cli.rs` | Small |
| 6.2 | Add `--non-interactive` flag (suppress TTY prompts, return errors instead) | `cli.rs` | Small |
| 6.3 | Make `--agent` = `--output json --non-interactive --ascii --no-thumb` (alias, backward compat) | `main.rs` | Small |
| 6.4 | Update docs to recommend `--output json` over `--agent` for new integrations | `README.md` | Small |
| 6.5 | Deprecation warning on `--agent` (emit to stderr) | `main.rs` | Small |

---

## Phase Dependency Graph

```
Phase 1 (Errors) ─────┬──→ Phase 3 (HATEOAS)
                       │
Phase 2 (Command Tree)─┘
                       
Phase 4 (Truncation)   [independent, can ship anytime]

Phase 5 (Streaming)    [independent, can ship anytime]

Phase 6 (Format Sep)   [independent, but cleaner after Phase 3]
```

Phases 1–2 can be parallelized. Phase 3 depends on both. Phases 4–6 are independent of each other.

---

## Estimated Total Effort

| Phase | Changes | Estimated Effort |
|-------|---------|-----------------|
| 1 | 6 | 2–3 days |
| 2 | 4 | 1–2 days |
| 3 | 20+ | 3–4 days |
| 4 | 7 | 2–3 days |
| 5 | 7 | 3–4 days |
| 6 | 5 | 1–2 days |
| **Total** | **49+** | **12–18 days** |

---

## What NOT to Do

- **Don't adopt "JSON only"** — zinc-cli serves humans at terminals. agcli can afford JSON-only because its users are purely agents. Zinc has a real `--features ui` dashboard.
- **Don't use agcli as a dependency** — zinc-cli has its own envelope format, its own command dispatch via clap, its own error taxonomy. Steal the ideas, not the crate.
- **Don't over-engineer `params`** — agcli's `ActionParam` with full `value`, `default`, `enum`, `required` is comprehensive but heavy. For zinc-cli, start with just `value` (pre-filled from context) and `required` (bool). Add `enum` and `default` only when specific commands need them.
- **Don't stream by default** — `--stream` as opt-in. Default behavior stays single-envelope. This avoids breaking every existing agent integration.

---

## Reference: agcli's 5 Core Principles

These inspired the phased plan above:

1. **JSON always** — every command returns structured JSON envelopes, never plain text
2. **HATEOAS** — every response includes `next_actions` telling the agent what to do next
3. **Self-documenting tree** — root command returns the full command tree as JSON
4. **Context protection** — truncation helpers cap large outputs with file pointers
5. **Errors suggest fixes** — error envelopes include `fix` and `retryable` fields

See: [agcli design.md](https://github.com/matthiasdebernardini/agcli/blob/master/design.md)
