#!/usr/bin/env bash
# PROTOTYPE launcher for issue #18.
# Builds the probe, wraps it in its own throwaway .app bundle and launches it with `open`, so macOS
# treats it as a separate app (the honest test for permission prompts such as Input Monitoring),
# instead of attributing it to the terminal. Usage: ./run.sh <label> [extra probe args]
set -euo pipefail
cd "$(dirname "$0")"

label="${1:-run}"
shift || true

cargo build --release

app="target/SDL3 Input Probe.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS"
cp target/release/sdl3-input-probe "$app/Contents/MacOS/sdl3-input-probe"
cat > "$app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleIdentifier</key><string>org.opendrone.prototype.sdl3-input-probe</string>
  <key>CFBundleName</key><string>SDL3 Input Probe</string>
  <key>CFBundleExecutable</key><string>sdl3-input-probe</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.0.0</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST
codesign --force --sign - "$app" >/dev/null 2>&1 || echo "(ad-hoc codesign failed; continuing)"

out="$(tty 2>/dev/null || true)"
if [[ -z "$out" || "$out" == "not a tty" ]]; then
  mkdir -p results
  out="$PWD/results/last-app-run.log"
  echo "No terminal attached; app output goes to $out"
fi

echo "Launching the probe as its own app. Follow the prompts in the probe window."
open -W -n --stdout "$out" --stderr "$out" "$app" --args --label "$label" "$@"
echo
echo "Probe closed. Newest results folder:"
ls -td results/"$label"-* 2>/dev/null | head -1
