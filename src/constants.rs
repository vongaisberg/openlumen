#![allow(dead_code)]
//! Centralized device identity / protocol constants.
//!
//! Keep these values in one place so Art-Net responses and the Web UI
//! report consistent firmware/manufacturer information.


/// Art-Net / ArtPollReply OEM code.
pub const ARTNET_OEM_CODE: u16 = 0x2908;

/// Art-Net / ArtPollReply ESTA manufacturer code.
pub const ARTNET_ESTA_MANUFACTURER: u16 = 0x0922;

/// Web UI firmware version as `[major, minor, patch]`.
pub const FIRMWARE_VERSION: [u8; 3] = [1, 0, 2];

/// Art-Net / ArtPollReply firmware version.
///
/// Written into `PollReply.firmware_version` (u16, big-endian).
pub const ARTNET_FIRMWARE_VERSION: u16 = 0x0102;


/// Web UI hardware version as `[major, minor, patch]`.
pub const HARDWARE_VERSION: [u8; 3] = [2, 0, 0]; // RP2350 hardware

/// Web UI device id string.
pub const DEVICE_ID: &str = "RP2350-ARTNET";

/// Web UI system status text.
pub const SYSTEM_STATUS: &str = "Running";

/// Version number for `settings.json` stored in flash.
/// Used to detect when stored settings are no longer compatible.
pub const SETTINGS_VERSION: u8 = 2;

