#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

echo "Cleaning previous build..."
rm -rf build

echo "Setting up Meson..."
meson setup build

echo "Compiling..."
meson compile -C build

echo "Build successful!"
