#!/bin/sh
set -eu

: "${MSRV:=1.88.0}"

cargo "+${MSRV}" check -p icloud-types --all-targets --locked
cargo "+${MSRV}" check -p icloud-protocol --all-targets --locked
cargo "+${MSRV}" check -p icloud-client --all-targets --no-default-features --locked
cargo "+${MSRV}" test --workspace --all-targets --all-features --locked
RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links" \
  cargo "+${MSRV}" doc --workspace --all-features --no-deps --locked
