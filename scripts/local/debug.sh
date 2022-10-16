#!/bin/bash
set -e
here=$(realpath $(dirname "$0"))
cd "$here/.."

cargo build

cd "$here"

cp ../../target/wasm32-unknown-unknown/debug/leaksignal.wasm .
docker-compose build --no-cache
rm -f leaksignal.wasm
docker-compose up
