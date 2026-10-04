#!/usr/bin/env bash
# PROTOTYPE launcher for issue #28: "Does the FPV camera look right and fit 2 ms on the M4?"
#
#   ./run.sh                   the app: Bando, fullscreen at native 2560x1440, tuning panel on the left
#   ./run.sh --skate           start on the Skate Park instead
#   ./run.sh --load NAME.toml  start from a tuning you saved (files in saved/)
#   ./run.sh --windowed        in a window instead of fullscreen
#   ./run.sh --pipelined       with Bevy's pipelined rendering ON (it is OFF by default, per #14)
#   ./run.sh --frames-in-flight 1   allow only 1 frame queued on the GPU (default 2, Bevy's default)
#   ./run.sh bench             GPU benchmark, about 2 minutes, writes results/bench-*.md, then quits
#   ./run.sh latency           stick-to-frame test: pipelining off/on x 1 or 2 frames in flight
#                              (about 70 s), writes results/latency-*.md, then quits
#   ./run.sh screenshots       screenshot set into results/screenshots/, then quits
#   ./run.sh profile [--load F]  Video Signal along the paths and at named Bando spots, per
#                              Breakup level (no window, a few seconds), into results/
# bench, latency and screenshots also take --offscreen (no window; works with the screen locked).
#
# Quit with Cmd+Q or Shift+Esc. Your last tuning is also autosaved to saved/autosave.toml.
set -euo pipefail
cd "$(dirname "$0")"

cargo build --release
bin=target/release/fpv-camera-look

case "${1:-}" in
  bench)
    shift
    exec "$bin" --bench "$@"
    ;;
  latency)
    shift
    # Pipelined rendering off/on, with 2 frames allowed on the GPU (Bevy's default) and with 1.
    "$bin" --latency --frames-in-flight 2 "$@"
    "$bin" --latency --frames-in-flight 2 --pipelined "$@"
    "$bin" --latency --frames-in-flight 1 "$@"
    "$bin" --latency --frames-in-flight 1 --pipelined "$@"
    echo
    echo "Reports: $(ls -t results/latency-*.md | head -4 | tr '\n' ' ')"
    ;;
  screenshots)
    shift
    exec "$bin" --screenshots "$@"
    ;;
  profile)
    shift
    exec "$bin" --signal-profile "$@"
    ;;
  *)
    exec "$bin" "$@"
    ;;
esac
