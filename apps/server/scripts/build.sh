#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH= cd -- "$script_dir/.." && pwd)

cd "$root"
mise exec -- cargo build --release --locked
echo "Build complete: $root/target/release/meloark-server"
