#!/bin/sh
# Builds the Tests tab to WebAssembly and puts it where the manifest's `native` says:
# tabs/tests.wasm. Hangar runs the module a kit ships and never builds one, so the module is
# committed, in the commit that changes its source. Run from anywhere.
#
#   rustup target add wasm32-unknown-unknown     once
#   scripts/build-native.sh
#   scripts/build-native.sh --check              fails if the committed module is not this build
#
# The module is the same bytes wherever it is built, with the Rust version of native/rust-version
# and the dependencies of native/Cargo.lock: the paths of the machine are remapped, so that CI
# can tell a stale module from a fresh one by comparing them.
set -eu
cd "$(dirname "$0")/.."
root=$(pwd)

want=$(cat native/rust-version)
have=$(rustc --version | cut -d' ' -f2)
if [ "$have" != "$want" ]; then
  echo "note: tabs/tests.wasm is built with rustc $want and this is $have: the module may differ byte for byte" >&2
fi

cargo_home=${CARGO_HOME:-$HOME/.cargo}
out=native/target/wasm32-unknown-unknown/release/kit_tests_tab.wasm
RUSTFLAGS="--remap-path-prefix=$root=/kit --remap-path-prefix=$cargo_home=/cargo" \
  cargo build --manifest-path native/Cargo.toml --release --locked --target wasm32-unknown-unknown

if [ "${1:-}" = "--check" ]; then
  if cmp -s "$out" tabs/tests.wasm; then
    echo "tabs/tests.wasm is up to date"
  else
    echo "tabs/tests.wasm is not what native/ builds: run scripts/build-native.sh and commit it" >&2
    exit 1
  fi
else
  cp "$out" tabs/tests.wasm
  echo "tabs/tests.wasm: $(wc -c < tabs/tests.wasm) bytes"
fi
