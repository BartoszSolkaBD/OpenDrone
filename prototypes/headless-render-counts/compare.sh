#!/usr/bin/env bash
# PROTOTYPE for issue #29. Are two runs' render counts identical?
#   ./compare.sh results/runs/a results/runs/b
# Compares frames.csv and passes.csv of every Map in the first directory, byte for byte. These
# files hold counts only (no timings). Exit 1 and show the first differences if anything differs.
set -euo pipefail
a="$1"
b="$2"
status=0
for dir in "$a"/*/; do
  map="$(basename "$dir")"
  for f in frames.csv passes.csv; do
    if [ ! -f "$b/$map/$f" ]; then
      echo "MISSING  $map/$f in $b"
      status=1
    elif cmp -s "$a/$map/$f" "$b/$map/$f"; then
      echo "SAME     $map/$f ($(($(wc -l < "$a/$map/$f") - 1)) rows)"
    else
      echo "DIFFERENT $map/$f"
      diff "$a/$map/$f" "$b/$map/$f" | head -20 || true
      status=1
    fi
  done
done
exit $status
