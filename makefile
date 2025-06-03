

dev:
	RUST_LOG=debug cargo run

release:
	cargo run --release

build:
	cargo build --release

install: build
	@echo "Installing sshr to /usr/local/bin"
	@mkdir -p /usr/local/bin
	@cp target/release/sshr /usr/local/bin/
	@chmod +x /usr/local/bin/sshr
	@echo "sshr installed successfully"

uninstall:
	@echo "Removing sshr"
	@rm -f /usr/local/bin/sshr
	@echo "sshr uninstalled"
