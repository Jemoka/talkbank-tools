.PHONY: test clippy golden parity-check clean

test:
	cargo nextest run -p clan-core
	cargo test --doc -p clan-core

clippy:
	cargo clippy -p clan-core --all-targets -- -D warnings

golden:
	cargo nextest run -p clan-core -- golden

parity-check:
	./scripts/parity-check.sh

parity-check-summary:
	./scripts/parity-check.sh --summary-only

clean:
	cargo clean -p clan-core
	rm -rf parity-results/
