#!/bin/sh
set -eu

cargo check -p icloud-types --lib
cargo check -p icloud-protocol --lib
cargo check -p icloud-client --lib --no-default-features
cargo check -p icloud-client --lib --no-default-features --features reqwest-async
cargo check -p icloud-client --lib --no-default-features --features reqwest-blocking
cargo check -p icloud-client --lib --no-default-features --features reqwest-async,reqwest-blocking
cargo check -p icloud-client --tests --no-default-features
cargo check -p icloud-client --tests --no-default-features --features reqwest-async
cargo check -p icloud-client --tests --no-default-features --features reqwest-blocking

cargo check -p icloud-types --lib --no-default-features --target thumbv7em-none-eabihf
cargo check -p icloud-protocol --lib --no-default-features --target thumbv7em-none-eabihf

cargo check -p icloud-types --lib --target wasm32-wasip2
cargo check -p icloud-protocol --lib --target wasm32-wasip2
cargo check -p icloud-client --lib --target wasm32-wasip2 --no-default-features
