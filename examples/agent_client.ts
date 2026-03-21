/**
 * Minimal zinc-cli contract client for AI agents (TypeScript).
 *
 * Usage:
 *   export ZINC_WALLET_PASSWORD='...'
 *   npx tsx examples/agent_client.ts
 */

import { spawnSync } from "node:child_process";

type Json = Record<string, unknown>;

class ZincCliError extends Error {
  envelope: Json;

  constructor(envelope: Json) {
    const error = (envelope.error ?? {}) as Json;
    super(
      `zinc-cli error type=${String(error.type)} exit_code=${String(
        error.exit_code
      )}: ${String(error.message)}`
    );
    this.envelope = envelope;
  }
}

function runZinc(args: string[], correlationId?: string): Json {
  const cmd = ["--agent", ...(correlationId ? ["--correlation-id", correlationId] : []), ...args];
  const result = spawnSync("zinc-cli", cmd, { encoding: "utf-8" });
  const stdout = (result.stdout ?? "").trim();

  if (!stdout) {
    throw new Error(`zinc-cli returned empty stdout (stderr=${(result.stderr ?? "").trim()})`);
  }

  const envelope = JSON.parse(stdout) as Json;
  if (envelope.ok !== true) {
    throw new ZincCliError(envelope);
  }
  return envelope;
}

function main(): void {
  const cid = "ts-agent-demo-001";
  const info = runZinc(["wallet", "info"], cid);
  console.log(
    `profile=${String(info.profile)} network=${String(info.network)} scheme=${String(info.scheme)}`
  );

  runZinc(["sync", "chain"], cid);
  runZinc(["sync", "ordinals"], cid);
  const balance = runZinc(["balance"], cid);
  const total = ((balance.total ?? {}) as Json).confirmed;
  console.log(`confirmed_sats=${String(total)}`);
}

main();
