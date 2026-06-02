.PHONY: fmt fmt-check clippy test feature-matrix shellcheck shfmt shfmt-check actionlint doc quality deny machete typos msrv minimal-versions ci-quality ci-msrv ci-minimal-versions

MSRV ?= 1.88.0
NIGHTLY ?= nightly

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all --check

clippy:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

test:
	cargo test --workspace --all-targets --all-features

feature-matrix:
	sh scripts/ci-feature-matrix.sh

shellcheck:
	shellcheck scripts/*.sh

shfmt:
	shfmt -w -i 2 scripts/*.sh

shfmt-check:
	shfmt -d -i 2 scripts/*.sh

actionlint:
	actionlint

doc:
	RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links" cargo doc --workspace --all-features --no-deps

quality: fmt-check clippy test feature-matrix doc

deny:
	cargo deny check advisories bans licenses sources

machete:
	cargo machete --skip-target-dir

typos:
	typos .

msrv:
	MSRV=$(MSRV) sh scripts/ci-msrv.sh

minimal-versions:
	MSRV=$(MSRV) NIGHTLY=$(NIGHTLY) sh scripts/ci-minimal-versions.sh

ci-quality: shellcheck shfmt-check actionlint quality deny machete typos

ci-msrv: msrv

ci-minimal-versions: minimal-versions
