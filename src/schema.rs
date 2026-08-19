use embassy_rp::otp;
use heapless::String;
use heapless::Vec;
use serde::{Deserialize, Serialize};

// This file contains the schemas for configuration and information transmitted to and from the frontend

pub const SETTINGS_VERSION: u8 = 2;

/// Consolidated settings structure stored in flash
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredSettings {
    pub version: u8,
    pub dmx_ports: [DmxPortConfig; 4],
    /// LED (WS281x) outputs. Defaulted when absent so that settings written by
    /// firmware predating LED support still load instead of triggering a reset.
    #[serde(rename = "ledPorts", default = "default_led_ports")]
    pub led_ports: [LedPortConfig; NUM_LED_PORTS],
    pub network_config: NetworkConfig,
    pub artnet_config: ArtnetConfig,
}

fn default_led_ports() -> [LedPortConfig; NUM_LED_PORTS] {
    core::array::from_fn(LedPortConfig::default_with_index)
}

impl Default for StoredSettings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            dmx_ports: core::array::from_fn(DmxPortConfig::default_with_universe),
            led_ports: default_led_ports(),
            network_config: NetworkConfig::default(),
            artnet_config: ArtnetConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub id: Option<u32>,
    #[serde(rename = "ipConfigType")]
    pub ip_config_type: IpConfigType,
    #[serde(rename = "ipAddress")]
    pub ip_address: Option<[u8; 4]>,
    #[serde(rename = "subnetMask")]
    pub subnet_mask: Option<[u8; 4]>,
    pub gateway: Option<[u8; 4]>,
    #[serde(rename = "macAddress")]
    pub mac_address: [u8; 6],
    #[serde(rename = "currentIpAddress")]
    pub current_ip_address: Option<[u8; 4]>,
    #[serde(rename = "currentSubnetMask")]
    pub current_subnet_mask: Option<[u8; 4]>,
    #[serde(rename = "currentGateway")]
    pub current_gateway: Option<[u8; 4]>,
}

impl Default for NetworkConfig {
    fn default() -> Self {

        // Generate mac address with fixed prefix and chip id
        // Format: 02:D5:12:XX:YY:ZZ
        //02 for locally administered address
        //D5:12 for DMX512
        //XX:YY:ZZ for unique identifier
        let mut mac_address = [0x02, 0xD5, 0x12, 0x00, 0x00, 0x00];
        let chip_id = otp::get_chipid().unwrap();

        mac_address[3] = ((chip_id >> 16) & 0xFF) as u8;
        mac_address[4] = ((chip_id >> 8) & 0xFF) as u8;
        mac_address[5] = (chip_id & 0xFF) as u8;

        let ip_address = Some([10, mac_address[3], mac_address[4], mac_address[5]]);

        Self {
            id: None,
            ip_config_type: IpConfigType::Static,
            ip_address: ip_address,
            subnet_mask: Some([255, 0, 0, 0]),
            gateway: Some([10, 0, 0, 1]),
            mac_address: mac_address,
            current_ip_address: None,
            current_subnet_mask: None,
            current_gateway: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum IpConfigType {
    #[serde(rename = "dhcp")]
    Dhcp,
    #[serde(rename = "static")]
    Static,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtnetConfig {
    pub id: Option<u32>,
    pub net: u8,
    pub subnet: u8,
    #[serde(rename = "deviceName")]
    pub device_name: String<32>,
}

impl Default for ArtnetConfig {
    fn default() -> Self {
        Self {
            id: None,
            net: 0,
            subnet: 0,
            device_name: String::try_from("OpenLumen Node").unwrap_or_default(),
        }
    }
}

/// Network configuration update item (editable fields only)
/// This struct is used for updates from the frontend and excludes read-only fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfigUpdateItem {
    #[serde(rename = "ipConfigType")]
    pub ip_config_type: IpConfigType,
    #[serde(rename = "ipAddress")]
    pub ip_address: Option<[u8; 4]>,
    #[serde(rename = "subnetMask")]
    pub subnet_mask: Option<[u8; 4]>,
    pub gateway: Option<[u8; 4]>,
    // Note: id, macAddress, and current_* fields are intentionally omitted - they're read-only
}

/// ArtNet configuration update item (editable fields only)
/// This struct is used for updates from the frontend and excludes read-only fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtnetConfigUpdateItem {
    pub net: u8,
    pub subnet: u8,
    #[serde(rename = "deviceName")]
    pub device_name: String<32>,
    // Note: id is intentionally omitted - it's read-only
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceDevice {
    pub name: String<17>,
    pub ip: [u8; 4],
    #[serde(rename = "packets_per_second")]
    pub packets_per_second: Option<u32>,
    /// Physical input port number (0 or 1). Used with MergeMode::Priority: lower value = primary.
    pub physical: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DmxPortConfig {
    pub mode: PortMode,
    pub universe: u8,
    #[serde(rename = "mergeMode")]
    pub merge_mode: MergeMode,
    #[serde(rename = "outputRate")]
    pub output_rate: OutputRate,
    #[serde(rename = "sourceDevices")]
    pub source_devices: Vec<SourceDevice, 2>,
    /// Whether a failsafe scene is stored for this port (read-only, set at runtime from failsafe store).
    #[serde(rename = "hasFailsafe", default)]
    pub has_failsafe: bool,
}

impl DmxPortConfig {
    pub fn default_with_universe(universe: usize) -> Self {
        Self {
            mode: PortMode::default(),
            universe: universe as u8,
            merge_mode: MergeMode::default(),
            output_rate: OutputRate::default(),
            source_devices: Vec::new(),
            has_failsafe: false,
        }
    }
}

/// DMX port configuration update item (editable fields only)
/// This struct is used for updates from the frontend and excludes read-only fields like sourceDevices
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DmxPortConfigUpdateItem {
    pub mode: PortMode,
    pub universe: u8,
    #[serde(rename = "mergeMode")]
    pub merge_mode: MergeMode,
    #[serde(rename = "outputRate")]
    pub output_rate: OutputRate,
    // Note: sourceDevices is intentionally omitted - it's read-only and reported by the backend
}

// ============================================================================
// LED (WS281x / WS2815B) outputs
// ============================================================================

/// Number of WS281x outputs. Bounded by the four state machines of PIO2 —
/// PIO0 runs DMX TX and PIO1 runs DMX RX, so there is no spare capacity.
pub const NUM_LED_PORTS: usize = 4;

/// Largest universe span a single LED output may claim.
pub const MAX_UNIVERSES_PER_LED_PORT: usize = 6;

/// Largest pixel count a single LED output may drive.
/// Six universes of a 4-byte pixel format: 6 * (512 / 4).
pub const MAX_PIXELS_PER_LED_PORT: usize = MAX_UNIVERSES_PER_LED_PORT * 128;

/// Channel ordering of an LED strip. Also determines bytes per pixel, so no
/// separate pixel-format field is needed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorOrder {
    /// 3 bytes/pixel, green first — plain WS2812B/WS2815B.
    #[serde(rename = "GRB")]
    Grb,
    /// 3 bytes/pixel, red first.
    #[serde(rename = "RGB")]
    Rgb,
    /// 4 bytes/pixel, green first — the usual WS2815B-RGBW ordering.
    #[serde(rename = "GRBW")]
    #[default]
    Grbw,
    /// 4 bytes/pixel, red first.
    #[serde(rename = "RGBW")]
    Rgbw,
}

impl ColorOrder {
    pub const fn bytes_per_pixel(self) -> usize {
        match self {
            ColorOrder::Grb | ColorOrder::Rgb => 3,
            ColorOrder::Grbw | ColorOrder::Rgbw => 4,
        }
    }

    /// Pixels that fit in one 512-channel universe without straddling the
    /// boundary. For a 4-byte format this divides evenly (128); for 3 bytes it
    /// is the conventional 170, leaving 2 channels unused.
    pub const fn pixels_per_universe(self) -> usize {
        512 / self.bytes_per_pixel()
    }

    /// Reorder one pixel's channels from wire order into the left-aligned u32
    /// the PIO shifts out MSB first.
    ///
    /// `src` is the raw DMX slice for this pixel, always interpreted as R,G,B[,W].
    pub fn to_word(self, src: &[u8], brightness_cap: u8) -> u32 {
        let scale = |v: u8| -> u32 {
            if brightness_cap == 255 {
                v as u32
            } else {
                ((v as u16 * brightness_cap as u16) / 255) as u32
            }
        };
        let r = scale(*src.first().unwrap_or(&0));
        let g = scale(*src.get(1).unwrap_or(&0));
        let b = scale(*src.get(2).unwrap_or(&0));
        let w = scale(*src.get(3).unwrap_or(&0));

        match self {
            // 3-byte formats sit in the top 24 bits (autopull threshold is 24).
            ColorOrder::Grb => (g << 24) | (r << 16) | (b << 8),
            ColorOrder::Rgb => (r << 24) | (g << 16) | (b << 8),
            ColorOrder::Grbw => (g << 24) | (r << 16) | (b << 8) | w,
            ColorOrder::Rgbw => (r << 24) | (g << 16) | (b << 8) | w,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LedPortMode {
    #[serde(rename = "Active")]
    Active,
    /// Output disabled; the state machine stops driving the pin.
    #[serde(rename = "Inactive")]
    #[default]
    Inactive,
    /// Output running but every pixel forced to zero.
    #[serde(rename = "Blackout")]
    Blackout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedPortConfig {
    pub mode: LedPortMode,
    /// Full 15-bit Art-Net port address of the first universe of this strip.
    /// Stored whole (net << 8 | subnet << 4 | universe) rather than as a 4-bit
    /// universe, so the four LED outputs can span more than 16 universes in
    /// total without colliding with the DMX ports.
    #[serde(rename = "startUniverse")]
    pub start_universe: u16,
    /// 1-based DMX start address within `start_universe`, as you would patch a
    /// fixture. The strip's data is a contiguous channel stream from here, so
    /// with a non-zero offset a pixel may straddle a universe boundary — that
    /// is handled, and is why the ingest buffer holds raw channels rather than
    /// pre-converted pixels.
    #[serde(rename = "startAddress", default = "default_start_address")]
    pub start_address: u16,
    /// Number of pixels on the strip. Zero disables the output. The universe
    /// span follows from this, the start address and the colour order; it is
    /// not configured separately, because a span that disagreed with the pixel
    /// count would only ever be a bug.
    #[serde(rename = "pixelCount")]
    pub pixel_count: u16,
    #[serde(rename = "colorOrder")]
    pub color_order: ColorOrder,
    /// Global scale applied to every channel, 255 = unmodified. Useful to cap
    /// current draw on a large installation.
    #[serde(rename = "brightnessCap")]
    pub brightness_cap: u8,
    /// Reverse the pixel order, so channel-stream pixel 0 lands at the far end
    /// of the strip. Saves re-patching when a run is physically wired from the
    /// wrong end. Applied per whole pixel, never per byte.
    #[serde(default)]
    pub reverse: bool,
}

fn default_start_address() -> u16 {
    1
}

impl LedPortConfig {
    pub fn default_with_index(index: usize) -> Self {
        Self {
            mode: LedPortMode::Inactive,
            // Leave universes 0..3 to the DMX ports and give each LED output a
            // non-overlapping block of its maximum span.
            start_universe: (4 + index * MAX_UNIVERSES_PER_LED_PORT) as u16,
            start_address: 1,
            pixel_count: 0,
            color_order: ColorOrder::default(),
            brightness_cap: 255,
            reverse: false,
        }
    }

    /// Effective pixel count, clamped to what the buffers can hold.
    pub fn effective_pixels(&self) -> usize {
        (self.pixel_count as usize).min(MAX_PIXELS_PER_LED_PORT)
    }

    /// Start address clamped to a legal 1..=512 DMX address.
    pub fn effective_start_address(&self) -> usize {
        (self.start_address.clamp(1, 512)) as usize
    }

    /// Length of this strip's channel stream in bytes.
    pub fn stream_len(&self) -> usize {
        self.effective_pixels() * self.color_order.bytes_per_pixel()
    }

    /// Absolute channel index of the strip's first byte, counting 512 channels
    /// per universe from universe 0. Used to intersect against incoming
    /// packets without special-casing universe boundaries.
    pub fn first_channel(&self) -> u32 {
        self.start_universe as u32 * 512 + (self.effective_start_address() as u32 - 1)
    }

    /// How many consecutive universes this output touches. A non-zero start
    /// address can push this one higher than it would be at address 1.
    pub fn universe_span(&self) -> usize {
        let len = self.stream_len();
        if len == 0 {
            return 0;
        }
        let first = self.first_channel();
        let last = first + len as u32 - 1;
        ((last / 512) - (first / 512) + 1) as usize
    }

    /// Intersect this strip's channel stream with one received universe.
    ///
    /// Returns `(stream_offset, packet_offset, length)`: copy `length` bytes
    /// from `data[packet_offset..]` into the strip's stream at `stream_offset`.
    /// `None` when the universe carries nothing for this output.
    pub fn intersect(&self, addr: u16, data_len: usize) -> Option<(usize, usize, usize)> {
        let len = self.stream_len();
        if len == 0 || data_len == 0 {
            return None;
        }
        let strip_start = self.first_channel();
        let strip_end = strip_start + len as u32; // exclusive
        let packet_start = addr as u32 * 512;
        let packet_end = packet_start + data_len as u32; // exclusive

        let overlap_start = strip_start.max(packet_start);
        let overlap_end = strip_end.min(packet_end);
        if overlap_start >= overlap_end {
            return None;
        }

        Some((
            (overlap_start - strip_start) as usize,
            (overlap_start - packet_start) as usize,
            (overlap_end - overlap_start) as usize,
        ))
    }

    /// True when this universe carries the strip's final byte, i.e. the frame
    /// is complete and the output can be kicked.
    pub fn is_last_universe(&self, addr: u16) -> bool {
        let len = self.stream_len();
        if len == 0 {
            return false;
        }
        let last_channel = self.first_channel() + len as u32 - 1;
        (last_channel / 512) == addr as u32
    }
}

/// LED port configuration update item (editable fields only)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedPortConfigUpdateItem {
    pub mode: LedPortMode,
    #[serde(rename = "startUniverse")]
    pub start_universe: u16,
    #[serde(rename = "startAddress", default = "default_start_address")]
    pub start_address: u16,
    #[serde(rename = "pixelCount")]
    pub pixel_count: u16,
    #[serde(rename = "colorOrder")]
    pub color_order: ColorOrder,
    #[serde(rename = "brightnessCap")]
    pub brightness_cap: u8,
    #[serde(default)]
    pub reverse: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedPortConfigUpdate {
    #[serde(rename = "type")]
    pub type_: &'static str,
    pub data: Vec<LedPortConfigUpdateItem, NUM_LED_PORTS>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DmxPortOutput {
    #[serde(rename = "portNumber")]
    pub port_number: u8,
    #[serde(rename = "dmxData")]
    pub dmx_data: Vec<u8, 512>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DmxOutputUpdate {
    #[serde(rename = "type")]
    pub type_: &'static str,
    pub data: Vec<DmxPortOutput, 4>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Copy, PartialEq, Eq)]
pub enum PortMode {
    #[serde(rename = "Active")]
    #[default]
    Active,
    #[serde(rename = "Inactive")]
    Inactive,
    #[serde(rename = "Blackout")]
    Blackout,
    #[serde(rename = "Input")]
    Input,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Copy)]
pub enum MergeMode {
    #[serde(rename = "Htp")]
    #[default]
    Htp,
    #[serde(rename = "Ltp")]
    Ltp,
    #[serde(rename = "Priority")]
    Priority,
}

#[derive(Debug, Clone, Default, Copy, Serialize, Deserialize)]
pub enum OutputRate {
    #[serde(rename = "Hz20")]
    Hz20,
    #[serde(rename = "Hz30")]
    Hz30,
    #[serde(rename = "Hz44")]
    #[default]
    Hz44,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub id: Option<u32>,
    #[serde(rename = "firmwareVersion")]
    pub firmware_version: [u8; 3],
    #[serde(rename = "hardwareVersion")]
    pub hardware_version: [u8; 3],
    pub uptime: u32,
    pub temperature: i32,
    #[serde(rename = "artnetTraffic")]
    pub artnet_traffic: u32,
    #[serde(rename = "packetLoss")]
    pub packet_loss: f32,
    #[serde(rename = "systemStatus")]
    pub system_status: String<32>,
    #[serde(rename = "deviceId")]
    pub device_id: String<32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateUpdate {
    #[serde(rename = "type")]
    pub type_: &'static str,
    pub data: StateUpdateData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateUpdateData {
    #[serde(rename = "networkConfig")]
    pub network_config: Option<NetworkConfig>,
    #[serde(rename = "artnetConfig")]
    pub artnet_config: Option<ArtnetConfig>,
    #[serde(rename = "dmxPorts")]
    pub dmx_ports: Option<Vec<DmxPortConfig, 4>>,
    #[serde(rename = "ledPorts")]
    pub led_ports: Option<Vec<LedPortStatus, NUM_LED_PORTS>>,
    #[serde(rename = "systemInfo")]
    pub system_info: Option<SystemInfo>,
}

/// An LED output as reported to the web UI: the stored config plus the
/// universe span derived from it, so the frontend does not have to duplicate
/// the pixels-per-universe arithmetic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedPortStatus {
    pub mode: LedPortMode,
    #[serde(rename = "startUniverse")]
    pub start_universe: u16,
    #[serde(rename = "startAddress")]
    pub start_address: u16,
    #[serde(rename = "pixelCount")]
    pub pixel_count: u16,
    #[serde(rename = "colorOrder")]
    pub color_order: ColorOrder,
    #[serde(rename = "brightnessCap")]
    pub brightness_cap: u8,
    pub reverse: bool,
    /// Number of universes this output consumes (read-only, derived).
    #[serde(rename = "universeSpan")]
    pub universe_span: u8,
    /// Highest pixel count the firmware buffers allow (read-only).
    #[serde(rename = "maxPixels")]
    pub max_pixels: u16,
}

impl From<&LedPortConfig> for LedPortStatus {
    fn from(cfg: &LedPortConfig) -> Self {
        Self {
            mode: cfg.mode,
            start_universe: cfg.start_universe,
            start_address: cfg.start_address,
            pixel_count: cfg.pixel_count,
            color_order: cfg.color_order,
            brightness_cap: cfg.brightness_cap,
            reverse: cfg.reverse,
            universe_span: cfg.universe_span() as u8,
            max_pixels: MAX_PIXELS_PER_LED_PORT as u16,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DmxPortConfigUpdate {
    #[serde(rename = "type")]
    pub type_: &'static str,
    pub data: Vec<DmxPortConfigUpdateItem, 4>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfigUpdate {
    #[serde(rename = "type")]
    pub type_: &'static str,
    pub data: NetworkConfigUpdateItem,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtnetConfigUpdate {
    #[serde(rename = "type")]
    pub type_: &'static str,
    pub data: ArtnetConfigUpdateItem,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfoUpdate {
    #[serde(rename = "type")]
    pub type_: &'static str,
    pub data: SystemInfo,
}

/// Payload for setFailsafe WebSocket command. port_number is 0-based port index (0-3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetFailsafeData {
    #[serde(rename = "portNumber")]
    pub port_number: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetFailsafeUpdate {
    #[serde(rename = "type")]
    pub type_: &'static str,
    pub data: SetFailsafeData,
}

/// Payload for deleteFailsafe WebSocket command. port_number is 0-based port index (0-3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteFailsafeData {
    #[serde(rename = "portNumber")]
    pub port_number: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteFailsafeUpdate {
    #[serde(rename = "type")]
    pub type_: &'static str,
    pub data: DeleteFailsafeData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypedMessage {
    #[serde(rename = "type")]
    pub type_: &'static str,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SystemAction {
    #[serde(rename = "restartDevice")]
    RestartDevice,
    #[serde(rename = "resetToDefaults")]
    ResetToDefaults,
    #[serde(rename = "factoryReset")]
    FactoryReset,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemActionUpdate {
    #[serde(rename = "type")]
    pub type_: &'static str,
    pub action: SystemAction,
}
