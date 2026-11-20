SBF_OUTPUT := $(CURDIR)/target/deploy

.PHONY: verify
verify:
	cargo build-sbf --manifest-path programs/airspace-escrow/Cargo.toml --sbf-out-dir "$(SBF_OUTPUT)"
	cargo build-sbf --manifest-path programs/airspace-deposit/Cargo.toml --sbf-out-dir "$(SBF_OUTPUT)"
	cargo build-sbf --manifest-path tests/test-hook-program/Cargo.toml --features allow --sbf-out-dir "$(SBF_OUTPUT)/allow"
	cp "$(SBF_OUTPUT)/allow/test_hook_program.so" "$(SBF_OUTPUT)/test_hook_allow.so"
	cargo build-sbf --manifest-path tests/test-hook-program/Cargo.toml --features deny --sbf-out-dir "$(SBF_OUTPUT)/deny"
	cp "$(SBF_OUTPUT)/deny/test_hook_program.so" "$(SBF_OUTPUT)/test_hook_deny.so"
	cargo test
	cargo fmt --all --check
	cargo clippy --all-targets -- -D warnings
