#!/bin/bash
set -e
here=$(realpath $(dirname "$0"))
cd "$here/../leaksignal"

cargo build --release

COMMIT=$(git rev-parse --verify --short HEAD)
DATE=$(date -u '+%Y_%m_%d_%H_%M_%S')

PREFIX="${DATE}_${COMMIT}"

WASM_FILE="../target/wasm32-unknown-unknown/release/leaksignal.wasm"


HASH=$(sha256sum -b ${WASM_FILE} | cut -d" " -f1)
HASH_FILE=./leaksignal.sha256

echo $HASH > $HASH_FILE

aws s3 cp $WASM_FILE s3://leakproxy/${PREFIX}/leaksignal.wasm
aws s3 cp $HASH_FILE s3://leakproxy/${PREFIX}/leaksignal.sha256

rm -f $HASH_FILE

URL="https://leakproxy.s3.us-west-2.amazonaws.com/${PREFIX}/leaksignal.wasm"
URL2="https://ingestion.app.leaksignal.com/s3/leakproxy/${PREFIX}/leaksignal.wasm"

echo "WASM URL: ${URL}"
echo "WASM URL_PROXY: ${URL2}"
echo "WASM SHA256: ${HASH}"
