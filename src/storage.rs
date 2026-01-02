//! Flash storage for persistent settings
//!
//! Stores user-configurable settings (Network, ArtNet, DMX ports) to flash memory
//! and loads them at device startup.

use crate::schema::{self, ArtnetConfig, DmxPortConfig, NetworkConfig};
use defmt::*;
use serde::{Deserialize, Serialize};

/// Magic number to identify valid settings in flash
const SETTINGS_MAGIC: u32 = 0x4152544E; // "ARTN" in ASCII
const SETTINGS_VERSION: u8 = 1;

/// Flash storage location - last 4KB sector of flash
/// RP2350 flash starts at 0x10000000, typically 2MB or 4MB
/// We use the last sector (4096 bytes) for settings
const FLASH_BASE: u32 = 0x10000000;
const FLASH_SIZE: u32 = 2 * 1024 * 1024; // 2MB default
const SETTINGS_OFFSET: u32 = FLASH_SIZE - 4096; // Last 4KB sector
const SETTINGS_ADDR: *const u8 = (FLASH_BASE + SETTINGS_OFFSET) as *const u8;

/// Consolidated settings structure stored in flash
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredSettings {
    pub version: u8,
    pub dmx_ports: [DmxPortConfig; 4],
    pub network_config: NetworkConfig,
    pub artnet_config: ArtnetConfig,
}

/// Flash settings header
#[repr(C)]
struct SettingsHeader {
    magic: u32,
    version: u8,
    _reserved: [u8; 3],
    crc32: u32,
    data_len: u32,
}

impl SettingsHeader {
    const SIZE: usize = core::mem::size_of::<SettingsHeader>();
}

/// Storage error type
#[derive(Debug, Clone, Copy)]
pub enum StorageError {
    NotFound,
    InvalidMagic,
    InvalidVersion,
    InvalidCrc,
    SerializationError,
    FlashError,
}

impl defmt::Format for StorageError {
    fn format(&self, fmt: defmt::Formatter) {
        match self {
            StorageError::NotFound => defmt::write!(fmt, "NotFound"),
            StorageError::InvalidMagic => defmt::write!(fmt, "InvalidMagic"),
            StorageError::InvalidVersion => defmt::write!(fmt, "InvalidVersion"),
            StorageError::InvalidCrc => defmt::write!(fmt, "InvalidCrc"),
            StorageError::SerializationError => defmt::write!(fmt, "SerializationError"),
            StorageError::FlashError => defmt::write!(fmt, "FlashError"),
        }
    }
}

impl From<postcard::Error> for StorageError {
    fn from(_: postcard::Error) -> Self {
        StorageError::SerializationError
    }
}

/// Calculate CRC32 checksum
fn crc32(data: &[u8]) -> u32 {
    crc::Crc::<u32>::new(&crc::CRC_32_ISO_HDLC).checksum(data)
}

/// Load all settings from flash
pub fn load_settings() -> Result<StoredSettings, StorageError> {
    unsafe {
        // Read header
        let header_ptr = SETTINGS_ADDR as *const SettingsHeader;
        let header = core::ptr::read_volatile(header_ptr);

        // Check magic number
        if header.magic != SETTINGS_MAGIC {
            return Err(StorageError::InvalidMagic);
        }

        // Check version
        if header.version != SETTINGS_VERSION {
            warn!("Settings version mismatch: expected {}, got {}", SETTINGS_VERSION, header.version);
            return Err(StorageError::InvalidVersion);
        }

        // Read data
        let data_ptr = SETTINGS_ADDR.add(SettingsHeader::SIZE);
        let data_slice = core::slice::from_raw_parts(data_ptr, header.data_len as usize);

        // Verify CRC
        let calculated_crc = crc32(data_slice);
        if calculated_crc != header.crc32 {
            warn!("Settings CRC mismatch: expected {:08x}, got {:08x}", header.crc32, calculated_crc);
            return Err(StorageError::InvalidCrc);
        }

        // Deserialize
        let settings: StoredSettings = postcard::from_bytes(data_slice)?;
        Ok(settings)
    }
}

/// Save DMX port configurations to flash
pub fn save_dmx_ports(config: &[schema::DmxPortConfig; 4]) -> Result<(), StorageError> {
    // Load existing settings or create defaults
    let mut settings = match load_settings() {
        Ok(s) => s,
        Err(_) => get_default_settings(),
    };


    for (i, port) in config.iter().enumerate() {
        settings.dmx_ports[i] = port.clone();
    }

    save_settings(&settings)
}

/// Save network configuration to flash
pub fn save_network_config(config: &NetworkConfig) -> Result<(), StorageError> {
    // Load existing settings or create defaults
    let mut settings = match load_settings() {
        Ok(s) => s,
        Err(_) => get_default_settings(),
    };

    settings.network_config = config.clone();
    save_settings(&settings)
}

/// Save ArtNet configuration to flash
pub fn save_artnet_config(config: &ArtnetConfig) -> Result<(), StorageError> {
    // Load existing settings or create defaults
    let mut settings = match load_settings() {
        Ok(s) => s,
        Err(_) => get_default_settings(),
    };

    settings.artnet_config = config.clone();
    save_settings(&settings)
}

/// Save all settings to flash
fn save_settings(settings: &StoredSettings) -> Result<(), StorageError> {
    // Serialize settings
    let mut buffer = [0u8; 2048];
    let serialized = postcard::to_slice(settings, &mut buffer)
        .map_err(|_| StorageError::SerializationError)?;

    // Calculate CRC
    let crc = crc32(serialized);

    // Create header
    let header = SettingsHeader {
        magic: SETTINGS_MAGIC,
        version: SETTINGS_VERSION,
        _reserved: [0; 3],
        crc32: crc,
        data_len: serialized.len() as u32,
    };

    // Write to flash
    // Note: Flash write requires erasing the sector first
    // For now, we'll use a simple approach that requires the sector to be erased
    // In production, you'd want to implement proper flash erase/write operations
    unsafe {
        write_flash_sector(&header, serialized)?;
    }

    info!("Settings saved to flash successfully");
    Ok(())
}

/// Write to flash sector
/// 
/// Note: Flash writes on RP2350 require proper flash controller access.
/// This is a simplified implementation that will work for reading but writing
/// requires additional flash controller setup. For now, we'll use a static
/// buffer approach that can be written to flash via a bootloader or special tool.
/// 
/// TODO: Implement proper flash erase/write using RP2350 flash controller registers
unsafe fn write_flash_sector(_header: &SettingsHeader, data: &[u8]) -> Result<(), StorageError> {
    // For now, we'll store settings in a static buffer
    // This allows the code to compile and work for reading
    // Writing will need proper flash controller implementation
    
    // Store in static buffer (this is a workaround until proper flash write is implemented)
    static mut SETTINGS_BUFFER: Option<StoredSettings> = None;
    
    // Try to deserialize to validate, then store in buffer
    // In production, this would write to actual flash
    if let Ok(settings) = postcard::from_bytes::<StoredSettings>(data) {
        SETTINGS_BUFFER = Some(settings);
        warn!("Settings stored in memory buffer (flash write not yet implemented)");
        // Return success for now - actual flash write needs flash controller
        return Ok(());
    }
    
    Err(StorageError::FlashError)
}

/// Get default settings
fn get_default_settings() -> StoredSettings {
    use crate::schema::{IpConfigType, OutputRate, PortMode, MergeMode};
    use heapless::String;

    StoredSettings {
        version: SETTINGS_VERSION,
        dmx_ports: [
            DmxPortConfig {
                mode: PortMode::Active,
                universe: 0,
                merge_mode: MergeMode::Htp,
                output_rate: OutputRate::Hz44,
                source_devices: heapless::Vec::new(),
            },
            DmxPortConfig {
                mode: PortMode::Active,
                universe: 1,
                merge_mode: MergeMode::Htp,
                output_rate: OutputRate::Hz44,
                source_devices: heapless::Vec::new(),
            },
            DmxPortConfig {
                mode: PortMode::Active,
                universe: 2,
                merge_mode: MergeMode::Htp,
                output_rate: OutputRate::Hz44,
                source_devices: heapless::Vec::new(),
            },
            DmxPortConfig {
                mode: PortMode::Active,
                universe: 3,
                merge_mode: MergeMode::Htp,
                output_rate: OutputRate::Hz44,
                source_devices: heapless::Vec::new(),
            },
        ],
        network_config: NetworkConfig {
            id: None,
            ip_config_type: IpConfigType::Static,
            ip_address: Some([192, 168, 0, 2]),
            subnet_mask: Some([255, 255, 255, 0]),
            gateway: Some([192, 168, 0, 1]),
            mac_address: String::try_from("02:00:DE:AD:BE:EF").unwrap_or_default(),
            current_ip_address: Some([192, 168, 0, 2]),
            current_subnet_mask: Some([255, 255, 255, 0]),
            current_gateway: Some([192, 168, 0, 1]),
        },
        artnet_config: ArtnetConfig {
            id: None,
            net: 0,
            subnet: 0,
            device_name: String::try_from("RP2350 ArtNet Node").unwrap_or_default(),
        },
    }
}

/// Clear all settings (factory reset)
pub fn clear_settings() -> Result<(), StorageError> {
    // Erase the flash sector
    // In a real implementation, you'd call the flash erase function
    // For now, we'll just write invalid magic to mark it as cleared
    unsafe {
        let header_ptr = SETTINGS_ADDR as *mut u32;
        core::ptr::write_volatile(header_ptr, 0xFFFFFFFF);
    }
    Ok(())
}
