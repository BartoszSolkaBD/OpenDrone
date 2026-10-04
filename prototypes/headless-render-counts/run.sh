#!/usr/bin/env bash
# PROTOTYPE launcher for issue #29: "Can Bevy render headless on GitHub's Linux runner and report
# per-frame render counts?" No window is opened; it works with the screen locked.
#
#   ./run.sh                       both Maps, 120 flight frames each, into results/latest/
#   ./run.sh --frames 30           a shorter flight
#   ./run.sh --maps bando          one Map (bando, skate-park)
#   ./run.sh --out results/runs/a  somewhere else
#   ./run.sh --threads 2           Bevy on 2 worker threads (counts must not change)
#   ./compare.sh DIR_A DIR_B       are two runs' counts identical? (exit 1 if not)
#
# Each Map writes frames.csv (one row per frame), passes.csv (per frame, per render pass),
# load.csv (loading, not compared), summary.md (totals, per-frame min / max / mean, times)
# and mid-flight.png (taken after the flight, to check the picture isn't blank).
set -euo pipefail
cd "$(dirname "$0")"

frames=120
maps="bando skate-park"
out=results/latest
extra=()
while [ $# -gt 0 ]; do
  case "$1" in
    --frames) frames="$2"; shift 2 ;;
    --maps) maps="$2"; shift 2 ;;
    --out) out="$2"; shift 2 ;;
    --threads) extra+=(--threads "$2"); shift 2 ;;
    *) echo "unknown argument $1" >&2; exit 2 ;;
  esac
done

./prepare-wgpu.sh
cargo build --release
bin=target/release/headless-render-counts

for map in $maps; do
  rm -rf "${out:?}/$map"
  start=$(date +%s)
  "$bin" --map "$map" --frames "$frames" --out "$out/$map" ${extra[@]+"${extra[@]}"}
  echo "$map: $(( $(date +%s) - start )) s (app start to exit)"
done
