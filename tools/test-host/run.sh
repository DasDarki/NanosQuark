#!/usr/bin/env sh
set -eu

root="$(cd "$(dirname "$0")/../.." && pwd)"
package="${1:-$root/packages/quark-test}"
build="$root/target/test-host"

mkdir -p "$build"
cargo build --release --manifest-path "$root/Cargo.toml"
cc -O2 -no-pie -rdynamic -I"$root/sdk/include" "$root/tools/test-host/host.c" "$root/sdk/linux/liblua.a" -lm -ldl -o "$build/host"
exec "$build/host" "$root/tools/test-host/host.lua" "$root/target/release" "$package" "${QUARK_TEST_TIMEOUT:-60}"
