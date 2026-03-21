#!/usr/bin/env python3
"""
Minimal zinc-cli contract client for AI agents.

Usage:
  export ZINC_WALLET_PASSWORD='...'
  python3 examples/agent_client.py
"""

from __future__ import annotations

import json
import subprocess
from typing import Any, Dict, List


class ZincCliError(RuntimeError):
    def __init__(self, envelope: Dict[str, Any]):
        self.envelope = envelope
        error = envelope.get("error", {})
        super().__init__(
            f"zinc-cli error type={error.get('type')} exit_code={error.get('exit_code')}: {error.get('message')}"
        )


def run_zinc(args: List[str], *, correlation_id: str | None = None) -> Dict[str, Any]:
    cmd = ["zinc-cli", "--agent"]
    if correlation_id:
        cmd += ["--correlation-id", correlation_id]
    cmd += args

    proc = subprocess.run(cmd, capture_output=True, text=True, check=False)
    stdout = proc.stdout.strip()
    if not stdout:
        raise RuntimeError(f"zinc-cli returned empty stdout (stderr={proc.stderr.strip()})")

    envelope = json.loads(stdout)
    if not envelope.get("ok", False):
        raise ZincCliError(envelope)
    return envelope


def main() -> None:
    cid = "py-agent-demo-001"
    info = run_zinc(["wallet", "info"], correlation_id=cid)
    print(f"profile={info.get('profile')} network={info.get('network')} scheme={info.get('scheme')}")

    # Read-only snapshot flow
    run_zinc(["sync", "chain"], correlation_id=cid)
    run_zinc(["sync", "ordinals"], correlation_id=cid)
    balance = run_zinc(["balance"], correlation_id=cid)
    confirmed = balance.get("total", {}).get("confirmed")
    print(f"confirmed_sats={confirmed}")


if __name__ == "__main__":
    main()
