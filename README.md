# `hallswap-contract`

## Installing

1. Install [Rust](https://www.rust-lang.org/tools/install) 1.44.1+
2. Install [Docker](https://docs.docker.com/get-docker/) for compiling and ensuring that builds have similar checksums
3. Install `wasm32-unknown-unknown` for rust

```sh
# Check rust versions
rustc --version
cargo --version
rustup target list --installed
# If `wasm32-unknown-unknown` is not listed, install it:
rustup target add wasm32-unknown-unknown
```

## Building

Run in the root of this project to produce an optimised build in the `artifacts` directory:

```sh
./build.sh
```

## Testing

See [`./tests`](./tests/README.md) for details of the full E2E test.
