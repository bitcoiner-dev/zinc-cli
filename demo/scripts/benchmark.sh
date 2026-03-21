#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DEMO_DIR="$ROOT_DIR/demo"
ARTIFACT_DIR="$DEMO_DIR/artifacts"
mkdir -p "$ARTIFACT_DIR"

RUNS=3
BENCH_ID="bench-$(date +%Y%m%d-%H%M%S)-$$"
OUT_JSON="$ARTIFACT_DIR/benchmark_${BENCH_ID}.json"
OUT_MD="$ARTIFACT_DIR/benchmark_${BENCH_ID}.md"

usage() {
  cat <<'USAGE'
Usage: ./demo/scripts/benchmark.sh [options]

Options:
  --runs <n>            Number of demo runs (default: 3)
  --output-json <path>  Benchmark summary JSON path
  --output-md <path>    Benchmark summary Markdown path
  -h, --help            Show help
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --runs)
      RUNS="${2:-}"
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

if ! [[ "$RUNS" =~ ^[0-9]+$ ]] || [[ "$RUNS" -lt 1 ]]; then
  echo "--runs must be an integer >= 1" >&2
  exit 1
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "Missing required command: jq" >&2
  exit 1
fi

RUN_DIR="$ARTIFACT_DIR/${BENCH_ID}"
mkdir -p "$RUN_DIR"

RESULTS_FILE="$RUN_DIR/run_paths.txt"
: >"$RESULTS_FILE"

echo "Running benchmark: ${RUNS} runs"

for i in $(seq 1 "$RUNS"); do
  run_id="${BENCH_ID}-r${i}"
  run_json="$RUN_DIR/${run_id}.json"
  run_md="$RUN_DIR/${run_id}.md"
  run_data="$RUN_DIR/${run_id}.data"

  echo "  [$i/$RUNS] run_demo"
  if "$DEMO_DIR/scripts/run_demo.sh" \
      --run-id "$run_id" \
      --output-json "$run_json" \
      --output-md "$run_md" \
      --data-dir "$run_data"; then
    :
  else
    # Keep going to collect a full benchmark picture even when a run fails.
    echo "    run failed: $run_id"
  fi
  echo "$run_json" >>"$RESULTS_FILE"
done

jq -Rs '
  split("\n") | map(select(length > 0))
' "$RESULTS_FILE" >"$RUN_DIR/run_paths.json"

jq -n \
  --arg bench_id "$BENCH_ID" \
  --arg created_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
  --argjson run_paths "$(cat "$RUN_DIR/run_paths.json")" \
  '
  def percentile($arr; $p):
    if ($arr | length) == 0 then null
    else
      ($arr | sort) as $s
      | ($s | length) as $n
      | (($n - 1) * ($p / 100.0) | floor) as $idx
      | $s[$idx]
    end;

  def avg($arr):
    if ($arr | length) == 0 then null
    else (($arr | add) / ($arr | length))
    end;

  ($run_paths | map(input)) as $runs
  | ($runs | map(.total_duration_ms)) as $durations
  | ($runs | map(select(.status == "passed")) | length) as $succeeded
  | {
      benchmark_id: $bench_id,
      created_at_utc: $created_at,
      runs: ($runs | length),
      succeeded: $succeeded,
      failed: (($runs | length) - $succeeded),
      duration_ms: {
        min: (if ($durations | length) > 0 then ($durations | min) else null end),
        p50: percentile($durations; 50),
        p95: percentile($durations; 95),
        max: (if ($durations | length) > 0 then ($durations | max) else null end),
        avg: avg($durations)
      },
      run_ids: ($runs | map(.run_id)),
      run_files: $run_paths
    }' $(cat "$RESULTS_FILE") >"$OUT_JSON"

{
  echo "# First 5 Minutes Benchmark"
  echo
  echo "- Benchmark ID: \`$BENCH_ID\`"
  echo "- Runs: \`$RUNS\`"
  echo "- Summary JSON: \`$OUT_JSON\`"
  echo
  echo "## Summary"
  echo
  echo "| Metric | Value |"
  echo "|---|---:|"
  jq -r '
    [
      ["succeeded", .succeeded],
      ["failed", .failed],
      ["min_ms", .duration_ms.min],
      ["p50_ms", .duration_ms.p50],
      ["p95_ms", .duration_ms.p95],
      ["avg_ms", .duration_ms.avg],
      ["max_ms", .duration_ms.max]
    ] | .[] | "| \(.[0] | tostring) | \(.[1] | tostring) |"
  ' "$OUT_JSON"
  echo
  echo "## Run Files"
  jq -r '.run_files[] | "- `\(. )`"' "$OUT_JSON"
} >"$OUT_MD"

echo "Wrote benchmark JSON: $OUT_JSON"
echo "Wrote benchmark Markdown: $OUT_MD"
