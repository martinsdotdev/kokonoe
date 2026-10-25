#!/usr/bin/env bash
# Regenerates the engine's browser bindings: kokonoe-wasm built for wasm32, then wasm-bindgen
# writing the JS glue and TypeScript into web/src/lib/engine. CI runs this and fails if the
# result differs from what is checked in.
set -euo pipefail
cd "$(dirname "$0")/.."

target=wasm32-unknown-unknown
version=$(cargo tree -p kokonoe-wasm -e normal --target "$target" --prefix none --locked |
	sed -n 's/^wasm-bindgen v\([^ ]*\).*/\1/p' | head -n 1)
if [ -z "$version" ]; then
	echo "kokonoe-wasm has no wasm-bindgen yet, so there are no bindings to generate."
	exit 0
fi

cargo build -p kokonoe-wasm --target "$target" --release --locked

# The CLI must match the crate's wasm-bindgen exactly, or it refuses the module.
if [ "$(wasm-bindgen --version 2>/dev/null)" != "wasm-bindgen $version" ]; then
	echo "Installing wasm-bindgen-cli $version to match kokonoe-wasm."
	cargo install wasm-bindgen-cli --version "$version" --locked
fi

wasm-bindgen --target web --out-dir web/src/lib/engine \
	"target/$target/release/kokonoe_wasm.wasm"
echo "Wrote the bindings for wasm-bindgen $version to web/src/lib/engine."
