use core::error::Error;

use bitflags::bitflags;

use super::PROTOCOLL_VERSION;

const ART_POLL_OPCODE: u16 = 0x2000;

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
    pub fn from_buffer(buf: &[u8]) -> Option<Self> {
        if buf.len() < 14 {
            return None;
        }
        if buf.len() >= 22 {
            Some(Self {
                flags: Flags::from_bits(buf[12]).unwrap(),
                diag_priority: buf[13],
                target_port_address_top: u16::from_be_bytes([buf[14], buf[15]]),
                target_port_address_bottom: u16::from_be_bytes([buf[16], buf[17]]),
                esta_manufacturer: u16::from_be_bytes([buf[18], buf[19]]),
                oem_code: u16::from_be_bytes([buf[20], buf[21]]),
            })
        } else {
            Some(Self {
                flags: Flags::from_bits(buf[12]).unwrap(),
                diag_priority: buf[13],
                target_port_address_top: 0,
                target_port_address_bottom: 0,
                esta_manufacturer: 0,
                oem_code: 0,
            })
        }
    }

    /// Write the Poll message to a buffer.
    pub fn to_buffer(&self, buf: &mut [u8]) {
        if buf.len() < 14 {
            panic!("Buffer too small");
        }
        buf[0..8].copy_from_slice(&"Art-Net\0".as_bytes());
        buf[8..9].copy_from_slice(&[0x20, 0x00]);
        buf[9..11].copy_from_slice(&PROTOCOLL_VERSION.to_be_bytes());
        buf[11] = self.flags.bits();
        buf[12] = self.diag_priority;
        buf[13..14].copy_from_slice(&self.target_port_address_top.to_be_bytes());
        buf[14..16].copy_from_slice(&self.target_port_address_bottom.to_be_bytes());
        buf[16..18].copy_from_slice(&self.esta_manufacturer.to_be_bytes());
        buf[18..20].copy_from_slice(&self.oem_code.to_be_bytes());

        
    }
}
