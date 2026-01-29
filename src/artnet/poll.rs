use bitflags::bitflags;

use super::{ArtNetError, PROTOCOLL_VERSION};

const ART_POLL_OPCODE: u16 = 0x2000;
/// Minimum buffer size for ArtPoll packet serialization
pub const ART_POLL_MIN_SIZE: usize = 20;

pub struct Poll {
    /// Set behaviour of Node.
    pub flags: Flags,
    /// The lowest priority of diagnostics message that should be sent.
    pub diag_priority: u8,
    /// Top of the range of Port-Addresses to be tested if Targeted Mode is active.
    pub target_port_address_top: u16,
    /// Bottom of the range of Port-Addresses to be tested if Targeted Mode is active.
    pub target_port_address_bottom: u16,
    /// The ESTA Manufacturer code.
    pub esta_manufacturer: u16,
    /// The Oem code of the device.
    pub oem_code: u16,
}

bitflags! {
    /// Server behaviour of Node.
    pub struct Flags: u8 {

        // 0 = Disable Targeted Mode.
        // 1 = Enable Targeted Mode.
        const Targeted = 0b00100000;
        /// 0 = Enable VLC transmission.
        /// 1 = Disable VLC transmission.
        const Vlc = 0b00010000;
        /// 0 = Diagnostic messages are broadcast.
        /// 1 = Diagnostics messages are unicast.
        const DiagnosticsBroadcast = 0b00001000;
        /// 0 = Do not send me diagnostics messages.
        /// 1 = Send me diagnostics messages.
        const SendDiagnostics = 0b00000100;
        /// 0 = Only send ArtPollReply in response to an ArtPoll or ArtAddress.
        /// 1 = Send ArtPollReply whenever Node conditions change.
        const ArtPollReply = 0b00000010;

    }
}

impl Poll {
    /// Parse a Poll message from a buffer.
    /// Returns None if the buffer is too small or contains invalid flags.
    pub fn from_buffer(buf: &[u8]) -> Option<Self> {
        if buf.len() < 14 {
            return None;
        }

        // Safely handle potentially invalid flags - use from_bits_truncate to ignore unknown bits
        let flags = Flags::from_bits_truncate(buf[12]);

        if buf.len() >= 22 {
            Some(Self {
                flags,
                diag_priority: buf[13],
                target_port_address_top: u16::from_be_bytes([buf[14], buf[15]]),
                target_port_address_bottom: u16::from_be_bytes([buf[16], buf[17]]),
                esta_manufacturer: u16::from_be_bytes([buf[18], buf[19]]),
                oem_code: u16::from_be_bytes([buf[20], buf[21]]),
            })
        } else {
            Some(Self {
                flags,
                diag_priority: buf[13],
                target_port_address_top: 0,
                target_port_address_bottom: 0,
                esta_manufacturer: 0,
                oem_code: 0,
            })
        }
    }

    /// Write the Poll message to a buffer.
    /// Returns the number of bytes written on success, or an error if buffer is too small.
    pub fn to_buffer(&self, buf: &mut [u8]) -> Result<usize, ArtNetError> {
        if buf.len() < ART_POLL_MIN_SIZE {
            return Err(ArtNetError::BufferTooSmall);
        }
        buf[0..8].copy_from_slice(b"Art-Net\0");
        buf[8] = 0x00; // Opcode low byte
        buf[9] = 0x20; // Opcode high byte
        buf[10..12].copy_from_slice(&PROTOCOLL_VERSION.to_be_bytes());
        buf[12] = self.flags.bits();
        buf[13] = self.diag_priority;
        buf[14..16].copy_from_slice(&self.target_port_address_top.to_be_bytes());
        buf[16..18].copy_from_slice(&self.target_port_address_bottom.to_be_bytes());
        buf[18..20].copy_from_slice(&self.esta_manufacturer.to_be_bytes());

        Ok(ART_POLL_MIN_SIZE)
    }
}

