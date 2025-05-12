use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub id: Option<i32>,
    pub ip_config_type: String,
    pub ip_address: Option<String>,
    pub subnet_mask: Option<String>,
    pub gateway: Option<String>,
    pub mac_address: String,
    pub current_ip_address: Option<String>,
    pub current_subnet_mask: Option<String>,
    pub current_gateway: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtnetConfig {
    pub id: Option<i32>,
    pub net: u8,
    pub subnet: u8,
    pub device_name: String,
    pub protocol_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceDevice {
    pub name: String,
    pub ip: String,
    pub packets_per_second: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DmxPortConfig {
    pub id: Option<i32>,
    pub port_number: u8,
    pub mode: String,
    pub universe: u8,
    pub merge_mode: String,
    pub output_rate: String,
    pub packets_per_second: u32,
    pub source_devices: Vec<SourceDevice>,
    pub channel_values: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub id: Option<i32>,
    pub firmware_version: String,
    pub hardware_version: String,
    pub uptime: String,
    pub temperature: String,
    pub memory_usage: u8,
    pub cpu_load: u8,
    pub artnet_traffic: u32,
    pub packet_loss: u8,
    pub system_status: String,
    pub device_id: String,
    pub current_firmware_version: String,
    pub latest_firmware_version: String,
} 