# Installation

Build and flash OpenLumen Node firmware on Raspberry Pi Pico 2 (RP2350) with W5500 Ethernet.

---

## Prerequisites

- **Rust** — nightly toolchain (see [rust-toolchain.toml](../rust-toolchain.toml); currently `nightly-2025-05-01`)
- **Target** — `thumbv8m.main-none-eabihf` (installed automatically via rust-toolchain)
- **Node.js** — for building the web UI (e.g. Node 20 LTS)
- **picotool** — for converting the ELF to UF2 and flashing ([pico-sdk](https://github.com/raspberrypi/pico-sdk) / pico-toolchain, or system package if available)

---

## Build steps

The firmware embeds the web UI at compile time. Build the web frontend first, then the firmware.

### 1. Build the web frontend

From the repository root:

```bash
cd web_frontend
npm ci
npm run build:client
cd ..
```

Output goes to `web_frontend/dist/public/` (index.html and assets). The Rust build expects these files to exist.

### 2. Build the firmware

```bash
cargo build --release
```

Binary: `target/thumbv8m.main-none-eabihf/release/openlumen`

### 3. Convert to UF2 (for flashing)

```bash
picotool uf2 convert target/thumbv8m.main-none-eabihf/release/openlumen -t elf target/openlumen.uf2 --family rp2350-arm-s
```

Or use the project script (builds and converts in one go):

```bash
./build_and_flash.sh
```

### 4. Flash the device

1. Put the Pico 2 in bootloader mode: hold **BOOTSEL**, press **RESET** (or replug USB while holding BOOTSEL).
2. Run:

   ```bash
   picotool load target/openlumen.uf2 -t uf2 -v -x
   ```

If a device is already in bootloader mode, `./build_and_flash.sh` will detect it and flash automatically after building.

---

## After flashing

1. Power the board (USB or external). If using DHCP, it will request an address; check your router or use a static IP (configured via the web UI after first access).
2. Open the node’s IP address in a browser (e.g. `http://192.168.1.x`). The web UI is served by the device.
3. Configure network, Art-Net net/subnet, and DMX ports as needed.

Settings are stored in flash and persist across reboots.
