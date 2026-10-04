#!/usr/bin/env bash
# PROTOTYPE for issue #29. Are two runs' render counts identical?
#   ./compare.sh results/runs/a results/runs/b
# Compares frames.csv and passes.csv of every Map in the first directory, byte for byte. These
# files hold counts only (no timings). When a file differs, names the columns that differ and
# by how much. Exit 1 if anything differs.
set -euo pipefail
a="$1"
b="$2"
status=0
for dir in "$a"/*/; do
  map="$(basename "$dir")"
  for f in frames.csv passes.csv; do
    if [ ! -f "$b/$map/$f" ]; then
      echo "MISSING   $map/$f in $b"
      status=1
    elif cmp -s "$a/$map/$f" "$b/$map/$f"; then
      echo "SAME      $map/$f ($(($(wc -l < "$a/$map/$f") - 1)) rows)"
    else
      echo "DIFFERENT $map/$f"
      python3 - "$a/$map/$f" "$b/$map/$f" <<'EOF'
import csv, sys
ra = list(csv.reader(open(sys.argv[1])))
rb = list(csv.reader(open(sys.argv[2])))
head = ra[0]
if head != rb[0] or len(ra) != len(rb):
    print(f"          different shape: {len(ra) - 1} vs {len(rb) - 1} rows")
    sys.exit()
cols = {}
for x, y in zip(ra[1:], rb[1:]):
    if x[:7] != y[:7] and head[1] != "frames":
        print(f"          rows differ in their key: {x[:7]} vs {y[:7]}")
        sys.exit()
    for i, (u, v) in enumerate(zip(x, y)):
        if u != v:
            c = cols.setdefault(head[i], [0, 0.0])
            c[0] += 1
            try:
                rel = abs(float(u) - float(v)) / max(abs(float(u)), abs(float(v)), 1.0)
                c[1] = max(c[1], rel)
            except ValueError:
                pass
for name, (n, rel) in cols.items():
    print(f"          {name}: {n} of {len(ra) - 1} rows differ, by up to {rel * 100:.1f}%")
EOF
      status=1
    fi
  done
done
exit $status
