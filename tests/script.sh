#!/bin/bash

ARCH=""
if [[ $(arch) = "arm64" ]]; then
  ARCH=-aarch64
fi

ARCH=$ARCH bun src/index.ts
