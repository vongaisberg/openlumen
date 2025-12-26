use serde::{Deserialize, Serialize};
use heapless::String as String;
use heapless::Vec as Vec;

// This file contains the schemas for configuration and information transmitted to and from the frontend

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
    pub mac_address: String<18>,
    #[serde(rename = "currentIpAddress")]
    pub current_ip_address: Option<[u8; 4]>,
    #[serde(rename = "currentSubnetMask")]
    pub current_subnet_mask: Option<[u8; 4]>,
    #[serde(rename = "currentGateway")]
    pub current_gateway: Option<[u8; 4]>,
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

#[derive(Debug, Clone, Serialize, Deserialize, Copy)]
pub enum PortMode {
    #[serde(rename = "Active")]
    Active,
    #[serde(rename = "Inactive")]
    Inactive,
    #[serde(rename = "Blackout")]
    Blackout,
}

#[derive(Debug, Clone, Serialize, Deserialize, Copy)]
pub enum MergeMode {
    #[serde(rename = "Htp")]
    Htp,
    #[serde(rename = "Ltp")]
    Ltp,
    #[serde(rename = "Priority")]
    Priority,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum OutputRate {
    #[serde(rename = "Hz20")]
    Hz20,
    #[serde(rename = "Hz30")]
    Hz30,
    #[serde(rename = "Hz44")]
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
    #[serde(rename = "systemInfo")]
    pub system_info: Option<SystemInfo>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypedMessage {
    #[serde(rename = "type")]
    pub type_: &'static str,
}