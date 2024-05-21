#!/bin/bash

ARCH=""
if [[ $(arch) = "arm64" ]]; then
  ARCH=-arm64
fi

echo "Building..."

docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/rust-optimizer${ARCH}:0.15.0
