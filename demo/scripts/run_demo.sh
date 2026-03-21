#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DEMO_DIR="$ROOT_DIR/demo"
ARTIFACT_DIR="$DEMO_DIR/artifacts"
mkdir -p "$ARTIFACT_DIR"

# shellcheck disable=SC1091
source "$DEMO_DIR/env.sh"

MNEMONIC="${MNEMONIC:-rotate text off rich waste jump grab doctor today renew fault exotic}"
TRANSFER_SATS="${TRANSFER_SATS:-1000}"
FEE_BUFFER_SATS="${FEE_BUFFER_SATS:-500}"
MIN_REQUIRED_SATS=$((TRANSFER_SATS + FEE_BUFFER_SATS))

RUN_ID=""
OUT_JSON=""
OUT_MD=""
DATA_DIR=""
KEEP_DATA_DIR=0

usage() {
  cat <<'USAGE'
Usage: ./demo/scripts/run_demo.sh [options]

Options:
  --run-id <id>          Custom run id (default: timestamp-based)
  --output-json <path>   Output JSON report path
  --output-md <path>     Output Markdown report path
  --data-dir <path>      Wallet data dir for run (default: demo/artifacts/data_<run_id>)
  --keep-data-dir        Keep run data directory after completion
  -h, --help             Show help
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --run-id)
      RUN_ID="${2:-}"
      shift 2
      ;;
    --output-json)
      OUT_JSON="${2:-}"
      shift 2
      ;;
    --output-md)
      OUT_MD="${2:-}"
      shift 2
      ;;
    --data-dir)
      DATA_DIR="${2:-}"
      shift 2
      ;;
    --keep-data-dir)
      KEEP_DATA_DIR=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "Missing required command: $1" >&2
    exit 1
  }
}

require_cmd cargo
require_cmd jq
require_cmd python3

now_ms() {
  python3 -c 'import time; print(int(time.time() * 1000))'
}

log() {
  printf '[%s] %s\n' "$(date +%H:%M:%S)" "$*"
}

truncate_msg() {
  local msg="$1"
  msg="${msg//$'\n'/ }"
  if [[ ${#msg} -gt 220 ]]; then
    printf '%s...' "${msg:0:220}"
  else
    printf '%s' "$msg"
  fi
}

if [[ -z "$RUN_ID" ]]; then
  RUN_ID="demo-$(date +%Y%m%d-%H%M%S)-$$"
fi

if [[ -z "$OUT_JSON" ]]; then
  OUT_JSON="$ARTIFACT_DIR/demo_${RUN_ID}.json"
fi
if [[ -z "$OUT_MD" ]]; then
  OUT_MD="$ARTIFACT_DIR/demo_${RUN_ID}.md"
fi
if [[ -z "$DATA_DIR" ]]; then
  DATA_DIR="$ARTIFACT_DIR/data_${RUN_ID}"
fi

if [[ ! -x "$ROOT_DIR/target/debug/zinc-cli" ]]; then
  log "Building zinc-cli debug binary (first run may take a while)..."
  cargo build --manifest-path "$ROOT_DIR/Cargo.toml"
  log "Build complete."
fi
ZINC_BIN="${ZINC_BIN:-$ROOT_DIR/target/debug/zinc-cli}"
if [[ ! -x "$ZINC_BIN" ]]; then
  echo "zinc-cli binary not found or not executable: $ZINC_BIN" >&2
  exit 1
fi

export ZINC_CLI_DATA_DIR="$DATA_DIR"
export ZINC_CLI_PROFILE="${ZINC_CLI_PROFILE:-demo}"
export ZINC_CLI_IDEMPOTENCY_KEY="${ZINC_CLI_IDEMPOTENCY_KEY:-}"
export ZINC_CLI_NETWORK_TIMEOUT_SECS="${ZINC_CLI_NETWORK_TIMEOUT_SECS:-20}"
export ZINC_CLI_NETWORK_RETRIES="${ZINC_CLI_NETWORK_RETRIES:-1}"
export ZINC_CLI_POLICY_MODE="${ZINC_CLI_POLICY_MODE:-warn}"

rm -rf "$DATA_DIR"
mkdir -p "$DATA_DIR"

CORRELATION_ID="demo-${RUN_ID}"
START_MS="$(now_ms)"

log "Starting demo run: $RUN_ID"
log "Network=$ZINC_CLI_NETWORK Scheme=$ZINC_CLI_SCHEME"
log "Esplora=$ZINC_CLI_ESPLORA_URL"
log "Ord=$ZINC_CLI_ORD_URL"
log "Data dir=$DATA_DIR"

TMP_STEPS="$(mktemp)"
trap 'rm -f "$TMP_STEPS"' EXIT

LAST_STDOUT=""
RUN_FAILED=0
TRANSFER_STATUS="not_attempted"
TRANSFER_TXID=""
TRANSFER_NOTE=""
ACCOUNT0_TOTAL_BEFORE=0
ACCOUNT0_SPENDABLE_BEFORE=0
ACCOUNT0_TOTAL_AFTER=0
ACCOUNT1_TOTAL_BEFORE=0
ACCOUNT1_TOTAL_AFTER=0
ACCOUNT1_PAYMENT=""

json_total_sats() {
  local json_input="$1"
  jq -r '[.total.immature, .total.trusted_pending, .total.untrusted_pending, .total.confirmed] | map(tonumber) | add' <<<"$json_input"
}

json_spendable_sats() {
  local json_input="$1"
  jq -r '[.spendable.immature, .spendable.trusted_pending, .spendable.untrusted_pending, .spendable.confirmed] | map(tonumber) | add' <<<"$json_input"
}

run_step() {
  local name="$1"; shift
  local started ended duration status ok err_type err_msg stderr stdout parsed summary
  local out_file err_file
  out_file="$(mktemp)"
  err_file="$(mktemp)"

  log "Running $name"
  started="$(now_ms)"
  if "$ZINC_BIN" --agent --correlation-id "$CORRELATION_ID" "$@" >"$out_file" 2>"$err_file"; then
    status=0
  else
    status=$?
  fi
  ended="$(now_ms)"
  duration=$((ended - started))

  stdout="$(cat "$out_file")"
  stderr="$(cat "$err_file")"
  rm -f "$out_file" "$err_file"

  if parsed="$(jq -c . <<<"$stdout" 2>/dev/null)"; then
    ok="$(jq -r '.ok // false' <<<"$parsed")"
    err_type="$(jq -r '.error.type // ""' <<<"$parsed")"
    err_msg="$(jq -r '.error.message // ""' <<<"$parsed")"
  else
    ok="false"
    err_type="internal"
    err_msg="stdout was not valid JSON"
    parsed="null"
  fi

  LAST_STDOUT="$stdout"

  local step_status="ok"
  if [[ "$status" -ne 0 || "$ok" != "true" ]]; then
    step_status="failed"
    RUN_FAILED=1
  fi

  if [[ "$step_status" == "ok" ]]; then
    log "[OK] $name (${duration} ms)"
  else
    if [[ -n "$err_type" || -n "$err_msg" ]]; then
      summary="$(truncate_msg "$err_type: $err_msg")"
    elif [[ -n "$stderr" ]]; then
      summary="$(truncate_msg "$stderr")"
    else
      summary="unknown failure"
    fi
    log "[FAIL] $name (${duration} ms) - $summary"
  fi

  jq -n \
    --arg name "$name" \
    --arg status "$step_status" \
    --argjson duration_ms "$duration" \
    --argjson exit_code "$status" \
    --arg ok "$ok" \
    --arg error_type "$err_type" \
    --arg error_message "$err_msg" \
    --arg stderr "$stderr" \
    --argjson stdout "$parsed" \
    --arg command "$*" \
    '{
      name: $name,
      status: $status,
      duration_ms: $duration_ms,
      exit_code: $exit_code,
      ok: ($ok == "true"),
      command: $command,
      error: {
        type: $error_type,
        message: $error_message
      },
      stderr: $stderr,
      stdout: $stdout
    }' >>"$TMP_STEPS"

  [[ "$step_status" == "ok" ]]
}

run_step_or_fail() {
  local name="$1"; shift
  if ! run_step "$name" "$@"; then
    return 1
  fi
  return 0
}

if ! run_step_or_fail "[1] Import wallet (dual/regtest)" wallet import --mnemonic "$MNEMONIC" --network regtest --scheme dual --overwrite; then true; fi
if [[ "$RUN_FAILED" -eq 0 ]]; then
  run_step_or_fail "[2] Wallet info" wallet info || true
fi
if [[ "$RUN_FAILED" -eq 0 ]]; then
  run_step_or_fail "[3] Sync chain" sync chain || true
fi
if [[ "$RUN_FAILED" -eq 0 ]]; then
  run_step_or_fail "[4] Sync ordinals" sync ordinals || true
fi
if [[ "$RUN_FAILED" -eq 0 ]]; then
  run_step_or_fail "[5] List inscriptions" inscription list || true
fi
if [[ "$RUN_FAILED" -eq 0 ]]; then
  run_step_or_fail "[6] Account 0 taproot" address taproot || true
fi
if [[ "$RUN_FAILED" -eq 0 ]]; then
  run_step_or_fail "[7] Account 0 payment" address payment || true
fi
if [[ "$RUN_FAILED" -eq 0 ]]; then
  run_step_or_fail "[8] Account 0 balance" balance || true
  ACCOUNT0_TOTAL_BEFORE="$(json_total_sats "$LAST_STDOUT")"
  ACCOUNT0_SPENDABLE_BEFORE="$(json_spendable_sats "$LAST_STDOUT")"
fi
if [[ "$RUN_FAILED" -eq 0 ]]; then
  run_step_or_fail "[9] Switch to account 1" account use --index 1 || true
fi
if [[ "$RUN_FAILED" -eq 0 ]]; then
  run_step_or_fail "[10] Account 1 balance (before)" balance || true
  ACCOUNT1_TOTAL_BEFORE="$(json_total_sats "$LAST_STDOUT")"
fi
if [[ "$RUN_FAILED" -eq 0 ]]; then
  run_step_or_fail "[11] Account 1 payment address" address payment || true
  ACCOUNT1_PAYMENT="$(jq -r '.address // ""' <<<"$LAST_STDOUT")"
fi
if [[ "$RUN_FAILED" -eq 0 ]]; then
  run_step_or_fail "[12] Switch back to account 0" account use --index 0 || true
fi

if [[ "$RUN_FAILED" -eq 0 ]]; then
  if [[ "$ACCOUNT0_SPENDABLE_BEFORE" -lt "$MIN_REQUIRED_SATS" ]]; then
    TRANSFER_STATUS="skipped"
    TRANSFER_NOTE="insufficient spendable sats for transfer"
    log "[SKIP] Transfer flow: $TRANSFER_NOTE"
  elif [[ -z "$ACCOUNT1_PAYMENT" || "$ACCOUNT1_PAYMENT" == "null" ]]; then
    TRANSFER_STATUS="skipped"
    TRANSFER_NOTE="missing account 1 payment address"
    log "[SKIP] Transfer flow: $TRANSFER_NOTE"
  else
    TRANSFER_STATUS="attempted"
    if run_step "[13] Create transfer PSBT" psbt create --to "$ACCOUNT1_PAYMENT" --amount-sats "$TRANSFER_SATS" --fee-rate 1; then
      unsigned_psbt="$(jq -r '.psbt // ""' <<<"$LAST_STDOUT")"
      if [[ -n "$unsigned_psbt" && "$unsigned_psbt" != "null" ]]; then
        run_step "[14] Analyze PSBT" psbt analyze --psbt "$unsigned_psbt" || true
        if [[ "$RUN_FAILED" -eq 0 ]]; then
          run_step "[15] Sign PSBT" psbt sign --psbt "$unsigned_psbt" --finalize || true
        fi
        if [[ "$RUN_FAILED" -eq 0 ]]; then
          signed_psbt="$(jq -r '.psbt // ""' <<<"$LAST_STDOUT")"
          if [[ -n "$signed_psbt" && "$signed_psbt" != "null" ]]; then
            run_step "[16] Broadcast transfer" psbt broadcast --psbt "$signed_psbt" || true
            if [[ "$RUN_FAILED" -eq 0 ]]; then
              TRANSFER_TXID="$(jq -r '.txid // ""' <<<"$LAST_STDOUT")"
              run_step "[17] Sync chain (account 0)" sync chain || true
              if [[ "$RUN_FAILED" -eq 0 ]]; then
                run_step "[18] Account 0 balance (after)" balance || true
                ACCOUNT0_TOTAL_AFTER="$(json_total_sats "$LAST_STDOUT")"
              fi
              if [[ "$RUN_FAILED" -eq 0 ]]; then
                run_step "[19] Switch to account 1" account use --index 1 || true
              fi
              if [[ "$RUN_FAILED" -eq 0 ]]; then
                run_step "[20] Sync chain (account 1)" sync chain || true
              fi
              if [[ "$RUN_FAILED" -eq 0 ]]; then
                run_step "[21] Account 1 balance (after)" balance || true
                ACCOUNT1_TOTAL_AFTER="$(json_total_sats "$LAST_STDOUT")"
              fi
            fi
          else
            RUN_FAILED=1
          fi
        fi
      else
        RUN_FAILED=1
      fi
    fi
  fi
fi

END_MS="$(now_ms)"
TOTAL_DURATION_MS=$((END_MS - START_MS))

if [[ "$RUN_FAILED" -eq 0 ]]; then
  RUN_STATUS="passed"
else
  RUN_STATUS="failed"
fi

STEPS_JSON="$(jq -s '.' "$TMP_STEPS")"

jq -n \
  --arg run_id "$RUN_ID" \
  --arg status "$RUN_STATUS" \
  --arg correlation_id "$CORRELATION_ID" \
  --arg created_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
  --arg data_dir "$DATA_DIR" \
  --arg network "$ZINC_CLI_NETWORK" \
  --arg scheme "$ZINC_CLI_SCHEME" \
  --arg esplora_url "$ZINC_CLI_ESPLORA_URL" \
  --arg ord_url "$ZINC_CLI_ORD_URL" \
  --argjson transfer_sats "$TRANSFER_SATS" \
  --argjson fee_buffer_sats "$FEE_BUFFER_SATS" \
  --arg transfer_status "$TRANSFER_STATUS" \
  --arg transfer_note "$TRANSFER_NOTE" \
  --arg transfer_txid "$TRANSFER_TXID" \
  --argjson account0_total_before "$ACCOUNT0_TOTAL_BEFORE" \
  --argjson account0_spendable_before "$ACCOUNT0_SPENDABLE_BEFORE" \
  --argjson account0_total_after "$ACCOUNT0_TOTAL_AFTER" \
  --argjson account1_total_before "$ACCOUNT1_TOTAL_BEFORE" \
  --argjson account1_total_after "$ACCOUNT1_TOTAL_AFTER" \
  --argjson total_duration_ms "$TOTAL_DURATION_MS" \
  --argjson steps "$STEPS_JSON" \
  '{
    run_id: $run_id,
    status: $status,
    created_at_utc: $created_at,
    correlation_id: $correlation_id,
    environment: {
      network: $network,
      scheme: $scheme,
      esplora_url: $esplora_url,
      ord_url: $ord_url,
      data_dir: $data_dir
    },
    transfer: {
      amount_sats: $transfer_sats,
      fee_buffer_sats: $fee_buffer_sats,
      status: $transfer_status,
      note: $transfer_note,
      txid: $transfer_txid
    },
    balances: {
      account0_total_before: $account0_total_before,
      account0_spendable_before: $account0_spendable_before,
      account0_total_after: $account0_total_after,
      account1_total_before: $account1_total_before,
      account1_total_after: $account1_total_after
    },
    steps: $steps,
    total_duration_ms: $total_duration_ms,
    summary: {
      step_count: ($steps | length),
      passed_steps: ($steps | map(select(.status == "ok")) | length),
      failed_steps: ($steps | map(select(.status == "failed")) | length)
    }
  }' >"$OUT_JSON"

{
  echo "# First 5 Minutes Evaluation"
  echo
  echo "- Run ID: \`$RUN_ID\`"
  echo "- Status: **$RUN_STATUS**"
  echo "- Total Duration: \`${TOTAL_DURATION_MS} ms\`"
  echo "- Correlation ID: \`$CORRELATION_ID\`"
  echo "- Data Dir: \`$DATA_DIR\`"
  echo
  echo "## Environment"
  echo
  echo "- Network: \`$ZINC_CLI_NETWORK\`"
  echo "- Scheme: \`$ZINC_CLI_SCHEME\`"
  echo "- Esplora URL: \`$ZINC_CLI_ESPLORA_URL\`"
  echo "- Ord URL: \`$ZINC_CLI_ORD_URL\`"
  echo
  echo "## Transfer"
  echo
  echo "- Status: \`$TRANSFER_STATUS\`"
  [[ -n "$TRANSFER_NOTE" ]] && echo "- Note: $TRANSFER_NOTE"
  [[ -n "$TRANSFER_TXID" ]] && echo "- Txid: \`$TRANSFER_TXID\`"
  echo
  echo "## Steps"
  echo
  echo "| Step | Status | Duration (ms) | Exit |"
  echo "|---|---|---:|---:|"
  jq -r '.steps[] | "| \(.name) | \(.status) | \(.duration_ms) | \(.exit_code) |"' "$OUT_JSON"
} >"$OUT_MD"

echo "Wrote JSON report: $OUT_JSON"
echo "Wrote Markdown report: $OUT_MD"

if [[ "$KEEP_DATA_DIR" -eq 0 ]]; then
  rm -rf "$DATA_DIR"
  log "Cleaned up data dir: $DATA_DIR"
else
  log "Kept data dir: $DATA_DIR"
fi

if [[ "$RUN_STATUS" != "passed" ]]; then
  log "Demo run failed."
  exit 1
fi

log "Demo run passed."
