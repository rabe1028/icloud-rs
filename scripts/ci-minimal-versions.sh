#!/bin/sh
set -eu

: "${MSRV:=1.88.0}"
: "${NIGHTLY:=nightly}"

lock_backup="$(mktemp)"
cp Cargo.lock "${lock_backup}"

cleanup() {
  mv "${lock_backup}" Cargo.lock
}

trap cleanup EXIT INT TERM

cargo "+${NIGHTLY}" generate-lockfile -Z direct-minimal-versions
cargo "+${MSRV}" check -p icloud-types --all-targets --locked
cargo "+${MSRV}" check -p icloud-protocol --all-targets --locked
cargo "+${MSRV}" check -p icloud-client --all-targets --no-default-features --locked
cargo "+${MSRV}" test --workspace --all-targets --all-features --locked
