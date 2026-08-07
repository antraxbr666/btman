#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

GSETTINGS_SCHEMA_DIR="$(pwd)/build/data" \
  meson devenv -C build src/btman
