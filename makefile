


dev:
	cargo run

debug:
	RUST_LOG=debug cargo run

release:
	cargo run --release

build:
	cargo build --release

install:
	cargo install --path .

uninstall:
	cargo uninstall sshr
