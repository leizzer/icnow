.PHONY: build install dist test clean

# Build the project normally
build:
	cargo build --release

# Install the binary to ~/.cargo/bin
install:
	cargo install --path .

# Build distribution archives using cargo-dist
dist:
	cargo dist build

# Run tests
test:
	cargo test

# Clean the target directory
clean:
	cargo clean
