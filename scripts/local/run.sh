#!/bin/bash
set -e
here=$(realpath $(dirname "$0"))
cd "$here/../.."

cargo build --release

cd "$here"

export ENVOY_YAML=.envoy.gen.yaml
ENVOY_CONFIG=${ENVOY_CONFIG:-./config/envoy.yaml}
envsubst < $ENVOY_CONFIG > ./config/$ENVOY_YAML

cp ../../target/wasm32-unknown-unknown/release/leaksignal.wasm .
docker-compose build --no-cache
rm -f leaksignal.wasm
docker-compose up
