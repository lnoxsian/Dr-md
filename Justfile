default: help

# Display all available commands in the Justfile
help:
    @just --list

# Fast build (debug profile)
build:
    cargo build

# Build egui_commonmark crate only (faster testing during crate edits)
build-crates:
    cargo build --manifest-path crates/egui_commonmark/Cargo.toml

# Build all workspace packages (application and all crates)
build-workspace:
    cargo build --workspace

# Fast release build non optimized
release:
    cargo build --release

# Slow build (fully optimized release profile)
optimized:
    cargo build --profile release-optimized

# Fast check (syntax and type checking without code gen)
check:
    cargo check

# Run in debug mode
run:
    cargo run

# Run in release mode
run-release:
    cargo run --release

# Run in release optimized mode
run-optimized:
    cargo run --profile release-optimized

# Run tests
test:
    cargo test

# Clean build artifacts
clean:
    cargo clean

# Check code formatting
fmt:
    cargo fmt --all -- --check

# Run linter
clippy:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

# Build the application container using Docker
docker-build:
    docker build -t dr-md .

# Export the compiled release binary from the Docker container to target/docker/dr-md
docker-export:
    docker build --target builder -t dr-md-builder .
    mkdir -p target/docker
    docker run --rm --entrypoint cat dr-md-builder /app/target/release/dr-md > target/docker/dr-md
    chmod +x target/docker/dr-md

# Update the application version in Cargo.toml and VERSION
update-version version:
    sed -i 's/^version = "[^"]*"/version = "{{version}}"/' Cargo.toml
    echo "APP_VERSION={{version}}" > VERSION
    echo "RUST_VERSION=$(rustc --version | awk '{print $2}')" >> VERSION

# Generate the various resolutions for application icons
generate-icons:
    python3 scripts/generate_assets.py --type icons

# Generate the dark and light mode logo assets
generate-logos:
    python3 scripts/generate_assets.py --type logo

# Generate all application assets (icons and logos)
generate-assets:
    python3 scripts/generate_assets.py --type all

# Package the application as a .deb package
package-deb:
    @./scripts/package_deb.sh

# Package the application as a .rpm package
package-rpm:
    @./scripts/package_rpm.sh
