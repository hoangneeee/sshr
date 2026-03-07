

dev:
	RUST_LOG=debug cargo run

release:
	cargo run --release

build:
	cargo build --release

publish: build
	@echo "Publishing sshr $(shell grep '^version' Cargo.toml | head -1 | sed 's/version = \"\(.*\)\"/\1/') to GitHub"
	git tag v$(shell grep '^version' Cargo.toml | head -1 | sed 's/version = \"\(.*\)\"/\1/')
	git push --tags
	@echo "sshr $(shell grep '^version' Cargo.toml | head -1 | sed 's/version = \"\(.*\)\"/\1/') published to GitHub"

publish-latest: build
	@echo "Publishing sshr latest to GitHub"
	git tag -d latest
	git tag latest
	git push --tags -f
	@echo "sshr latest published to GitHub"

republish:
	@VERSION=$$(grep '^version' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/'); \
	echo "Republishing v$$VERSION and latest..."; \
	git tag -d v$$VERSION 2>/dev/null || true; \
	git push origin :refs/tags/v$$VERSION 2>/dev/null || true; \
	git tag v$$VERSION; \
	git push origin v$$VERSION; \
	git tag -d latest 2>/dev/null || true; \
	git push origin :refs/tags/latest 2>/dev/null || true; \
	git tag latest; \
	git push origin latest; \
	echo "Done: v$$VERSION and latest republished."

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

install-brew:
	brew tap hoangneeee/sshr
	brew install sshr

