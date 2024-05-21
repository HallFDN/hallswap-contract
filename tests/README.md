# Tests

This directory contains the full E2E tests for `Hallswap` against [`LocalTerra`](https://github.com/terra-money/localterra).

## Running E2E Tests

### Setup

```sh
# Clone the repo
git clone --depth 1 https://github.com/terra-money/LocalTerra

# Change into the dir
cd LocalTerra

# Start the localnet
docker compose up
```

Then, while `localterra` is running, run the test script:

### Running

1. Run `./build.sh` in the root of the project to build the wasm files
2. Then, run `./script.sh` and observe the results
