#!/bin/bash
set -e

echo "Building Aura VCS..."

# Build the core and server
echo "Building Rust components..."
cargo build --release --manifest-path vcs-core/Cargo.toml
cargo build --release --manifest-path server/Cargo.toml

# Build the client
echo "Building React client..."
cd client
npm install
npm run build
cd ..

echo "Build complete."
