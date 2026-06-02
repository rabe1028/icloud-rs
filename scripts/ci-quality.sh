#!/bin/sh
set -eu

if command -v shellcheck >/dev/null 2>&1; then
  shellcheck scripts/*.sh
fi

if command -v shfmt >/dev/null 2>&1; then
  shfmt -d -i 2 scripts/*.sh
fi

if command -v actionlint >/dev/null 2>&1; then
  actionlint
fi

cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
sh scripts/ci-feature-matrix.sh
RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links" cargo doc --workspace --all-features --no-deps

if command -v cargo-deny >/dev/null 2>&1; then
  cargo deny check advisories bans licenses sources
fi

if command -v cargo-machete >/dev/null 2>&1; then
  cargo machete --skip-target-dir
fi

if command -v typos >/dev/null 2>&1; then
  typos .
fi
