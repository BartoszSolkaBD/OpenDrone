#!/usr/bin/env bash
# PROTOTYPE launcher for issue #34: "Does the live Quad sound feel right, on board and from where
# you stand?" Throwaway. See README.md.
#
#   ./run.sh                    the app: fly and listen, tuning panel on the right
#   ./run.sh --load NAME.toml   start from a saved tuning (files in tuning/)
#   ./run.sh --no-device        the app as if there were no sound device (runs silent, says so once)
#   ./run.sh render [names]     the listening files: renders/*.wav, plus renders/m4a/*.m4a copies
#   ./run.sh bench              CPU cost and the live block size and output delay (silent), into results/bench.md
#   ./run.sh nodevice           the no-sound-device path, headless (a few seconds)
#   ./run.sh fetch-clips        download and checksum the CC0 clips (done automatically the first time)
#
# Your tuning is saved with "Save tuning" to tuning/current.toml (loaded next time) and a dated copy.
set -euo pipefail
cd "$(dirname "$0")"

if [ ! -d assets/clips/hits ]; then
  echo "Fetching the CC0 clips (about 25 MB, a few minutes, first time only)..."
  ./assets/fetch-clips.sh
fi

cargo build --release
bin=target/release/quad-sound

case "${1:-}" in
  render)
    shift
    "$bin" render "$@"
    mkdir -p renders/m4a
    for f in renders/*.wav; do
      afconvert -f m4af -d aac -b 256000 "$f" "renders/m4a/$(basename "${f%.wav}").m4a"
    done
    echo "Rendered: renders/*.wav and renders/m4a/*.m4a (see renders/README.md)"
    ;;
  bench | nodevice | analyze | cliplevels)
    exec "$bin" "$@"
    ;;
  fetch-clips)
    exec ./assets/fetch-clips.sh
    ;;
  *)
    exec "$bin" "$@"
    ;;
esac
