#!/bin/bash
# Build script for OpenLumen Node
# Builds the project, converts to UF2, and optionally flashes

set -e

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET="thumbv8m.main-none-eabihf"
BINARY_NAME="openlumen"
BUILD_DIR="$PROJECT_DIR/target/$TARGET/release"
UF2_FILE="$PROJECT_DIR/target/${BINARY_NAME}.uf2"

echo "Building OpenLumen Node..."
cargo build --release

echo "Converting to UF2..."
picotool uf2 convert "$BUILD_DIR/$BINARY_NAME" -t elf "$UF2_FILE" --family rp2350-arm-s

echo "Build complete! UF2 file: $UF2_FILE"

# Check if device is in bootloader mode
if picotool info &>/dev/null; then
    echo "Device found in bootloader mode. Flashing..."
    picotool load "$UF2_FILE" -t uf2 -v -x
    echo "Flash complete!"
else
    echo "No device in bootloader mode. To flash manually:"
    echo "  1. Hold BOOTSEL button and press RESET (or replug USB while holding BOOTSEL)"
    echo "  2. Run: picotool load $UF2_FILE -t uf2 -v -x"
fi
