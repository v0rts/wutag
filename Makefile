PROJECT := wutag


.PHONY: all
all: clean test build


.PHONY: all_debug
all_debug: clean test build_debug


.PHONY: run_debug
run_debug: build_debug
	@./target/debug/$(PROJECT)


.PHONY: run
run: build
	@./target/release/$(PROJECT)


.PHONY: build_debug
build_debug: ./target/debug/$(PROJECT)


.PHONY: build
build: ./target/release/$(PROJECT)


.PHONY: lint
lint: fmt_check clippy

.PHONY: check
check:
	cargo check --all

.PHONY: test
test:
	cargo t --all-targets --all-features


.PHONY: fmt_check
fmt_check:
	cargo fmt --all -- --check


.PHONY: fmt
fmt:
	cargo fmt --all


.PHONY: clippy
clippy:
	@rustup component add clippy
	cargo clippy --all-targets --all-features -- -D clippy::all


.PHONY: clean
clean:
	@rm -rf target/*


./target/debug/$(PROJECT):
	cargo build --bins


./target/release/$(PROJECT):
	cargo build --release --bins

