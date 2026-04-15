.PHONY: build test lint clean

build:
	swift build --package-path src/hydration
	swift build --package-path src/app
	swift build --package-path src/helper
	cd src/fuse && cargo build

test:
	swift test --package-path src/hydration
	cd src/fuse && cargo test

lint:
	swiftlint lint src/
	cd src/fuse && cargo clippy -- -D warnings
	ruff check scripts/

clean:
	swift package --package-path src/hydration clean
	swift package --package-path src/app clean
	swift package --package-path src/helper clean
	cd src/fuse && cargo clean
