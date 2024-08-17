use super::poll_reply::{GoodInput, GoodOutputA, GoodOutputB, PortTypes, Status1, Status2, Status3, StyleCode};

struct NodeConfig {
    pub esta_manufacturer: u16,
    pub oem_code: u16,
    pub firmware_version: u16,
    pub ubea_version: u8,
    pub num_ports: u16,
    pub style: StyleCode,
    pub refresh_rate: u16,

    pub port_name: [u8; 18],
    pub long_name: [u8; 64],

    pub mac_address: [u8; 6],
    pub ip_address: [u8; 4],
    pub port: u16,

    pub node_report: [u8; 64],
    pub status1: Status1,
    pub status2: Status2,
    pub status3: Status3,

    pub net: u8,
    pub sub_net: u8,
    pub port_types: [PortTypes; 4],
    pub good_input: [GoodInput; 4],
    pub good_output_a: [GoodOutputA; 4],
    pub good_output_b: [GoodOutputB; 4],

    pub sw_in: [u8; 4],
    pub sw_out: [u8; 4],
    pub acn_priority: u8,


}