#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

echo "=== btman build ==="

# Clean previous build
rm -rf build

# Create build directories
mkdir -p build/src build/data

# Get version from Cargo.toml
VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
echo "Version: $VERSION"

# Generate config.rs
cat > src/config.rs << EOF
pub static VERSION: &str = "$VERSION";
EOF
echo "Generated src/config.rs"

# Compile Blueprint files
echo "Compiling Blueprints..."
blueprint-compiler batch-compile build/src/gtk src/gtk \
    src/gtk/window.blp \
    src/gtk/device-action-row.blp \
    src/gtk/startup-error-message.blp

# Copy CSS to build
cp src/gtk/style.css build/src/gtk/style.css

# Compile GResources
echo "Compiling GResources..."
glib-compile-resources --sourcedir build/src --target build/src/btman.gresource src/btman.gresource.xml

# Compile GSettings schema
echo "Compiling GSettings schema..."
glib-compile-schemas --strict --targetdir build/data data/

echo "Pre-build steps done!"

# Build with Cargo
echo "Building with Cargo..."
cargo build "$@"

# Copy gresource next to binary
cp build/src/btman.gresource target/debug/btman.gresource 2>/dev/null || true

echo "=== Build successful! ==="
