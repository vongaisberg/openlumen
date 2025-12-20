#!/bin/bash
# Cargo runner script - called by `cargo run`
# $1 is the path to the built ELF file

set -e

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ELF_FILE="$1"
UF2_FILE="$PROJECT_DIR/target/artnet_node.uf2"

echo "Converting to UF2..."
picotool uf2 convert "$ELF_FILE" -t elf "$UF2_FILE" --family rp2350-arm-s

echo "UF2 created: $UF2_FILE"

# Check if device is in bootloader mode
if picotool info &>/dev/null; then
    echo "Device found. Flashing..."
    picotool load "$UF2_FILE" -t uf2 -v -x
    echo "Flash complete!"
else
    echo "No device in bootloader mode."
    echo "To flash: Hold BOOTSEL + RESET, then run:"
    echo "  picotool load $UF2_FILE -t uf2 -v -x"
    exit 1
fi
