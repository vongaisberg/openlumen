#![allow(dead_code)]

pub mod poll;
pub mod poll_reply;
mod node_config;
pub mod dmx;

const PROTOCOLL_VERSION: u16 = 14;

// ArtNet Opcodes (Little Endian)
pub const OPCODE_POLL: u16 = 0x2000;
pub const OPCODE_POLL_REPLY: u16 = 0x2100;
pub const OPCODE_DMX: u16 = 0x5000;
// Add other opcodes as needed

/// Art-Net parsing/serialization errors
#[derive(Debug, Clone, Copy, PartialEq, Eq, defmt::Format)]
pub enum ArtNetError {
    /// Buffer is too small to hold the packet
    BufferTooSmall,
    /// Packet data is malformed or incomplete
    MalformedPacket,
    /// Invalid flags or bitfield value
    InvalidFlags,
    /// DMX length exceeds maximum (512)
    DmxLengthTooLarge,
}
