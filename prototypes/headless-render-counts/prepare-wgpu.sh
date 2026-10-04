#!/usr/bin/env bash
# PROTOTYPE for issue #29. Makes vendor/wgpu: wgpu 29.0.4 as published on crates.io, checked
# against its checksum, plus wgpu-counting.patch (our per-frame counting). Cargo.toml points
# Bevy's wgpu at it. Does nothing when vendor/wgpu already has this exact patch.
set -euo pipefail
cd "$(dirname "$0")"

version=29.0.4
sha256=76e8840e1ba2881d4cbb18d2147627a56af426ff064c0401eb0c8410c6325d07
stamp="$(shasum -a 256 wgpu-counting.patch | cut -d' ' -f1)"

if [ -f vendor/wgpu/.od-patch ] && [ "$(cat vendor/wgpu/.od-patch)" = "$stamp" ]; then
  exit 0
fi

rm -rf vendor/wgpu "vendor/wgpu-$version"
mkdir -p vendor
curl -sSfL "https://static.crates.io/crates/wgpu/wgpu-$version.crate" -o "vendor/wgpu-$version.crate"
echo "$sha256  vendor/wgpu-$version.crate" | shasum -a 256 -c - >/dev/null
tar -xzf "vendor/wgpu-$version.crate" -C vendor
mv "vendor/wgpu-$version" vendor/wgpu
patch -s -p1 -d vendor/wgpu < wgpu-counting.patch
echo "$stamp" > vendor/wgpu/.od-patch
echo "vendor/wgpu: wgpu $version + wgpu-counting.patch"
