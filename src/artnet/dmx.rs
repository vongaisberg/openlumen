use super::ArtNetError;

const ART_DMX_OPCODE: u16 = 0x5000;

#[derive(Debug)]
pub struct ArtDmx {
    /// The sequence number is used to ensure that ArtDmx packets are used in the correct order.
    pub sequence: u8,
    /// The physical input port from which this data was received
    pub physical: u8,
    /// The low byte of the 15 bit Port-Address to which this packet is destined.
    pub sub_uni: u8,
    /// The top 7 bits of the 15 bit Port-Address to which this packet is destined.
    pub net: u8,
    /// The length of the DMX512 data array
    pub length: u16,
    /// Array of DMC512 lightning data
    pub data: [u8; 512],
}

impl ArtDmx {
    /// Parse an ArtDmx packet from a buffer.
    /// Returns None if the buffer is too small or malformed.
    pub fn from_buffer(buf: &[u8]) -> Option<Self> {
        // Minimum packet size: 8 (header) + 2 (opcode) + 2 (version) + 1 (seq) + 1 (phys) + 2 (universe) + 2 (length) = 18
        // Plus at least 1 byte of data
        if buf.len() < 18 {
            return None;
        }

        let length = u16::from_be_bytes([buf[16], buf[17]]) as usize;

        // Validate DMX length: must be > 0 and <= 512
        if length == 0 || length > 512 {
            return None;
        }

        // Validate buffer has enough data: header (18 bytes) + DMX data (length bytes)
        if buf.len() < 18 + length {
            return None;
        }

        let mut res = Self {
            sequence: buf[12],
            physical: buf[13],
            sub_uni: buf[14],
            net: buf[15],
            length: length as u16,
            data: [0; 512],
        };

        // Safe: we validated that buf.len() >= 18 + length and length <= 512
        res.data[..length].copy_from_slice(&buf[18..18 + length]);

        Some(res)
    }
    pub fn default() -> Self {
        Self {
            sequence: 0,
            physical: 0,
            sub_uni: 0,
            net: 0,
            length: 0,
            data: {
                let mut data = [0; 512];
                data[0] = 0;
                data[1] = 85;
                data[7] = 85;
                data
            },
        }
    }

    pub fn universe(&self) -> u16 {
        ((self.net as u16) << 8) | (self.sub_uni as u16)
    }

    /// Serialize an ArtDmx packet into a buffer.
    ///
    /// Returns the number of bytes written, or an error if the buffer is too small.
    /// Packet layout:
    ///   [0..8]   "Art-Net\0" header
    ///   [8..10]  OpCode 0x5000 (little-endian)
    ///   [10..12] Protocol version 0x000E (big-endian)
    ///   [12]     Sequence
    ///   [13]     Physical
    ///   [14]     SubUni (low byte of port-address)
    ///   [15]     Net (high byte of port-address)
    ///   [16..18] Length (big-endian)
    ///   [18..]   DMX data
    pub fn to_buffer(&self, buf: &mut [u8]) -> Result<usize, ArtNetError> {
        let length = self.length as usize;
        let total = 18 + length;
        if buf.len() < total {
            return Err(ArtNetError::BufferTooSmall);
        }
        if length > 512 {
            return Err(ArtNetError::DmxLengthTooLarge);
        }

        // Art-Net header
        buf[0..8].copy_from_slice(b"Art-Net\0");
        // OpCode (little-endian)
        buf[8..10].copy_from_slice(&ART_DMX_OPCODE.to_le_bytes());
        // Protocol version (big-endian)
        buf[10..12].copy_from_slice(&0x000Eu16.to_be_bytes());
        // Sequence, Physical
        buf[12] = self.sequence;
        buf[13] = self.physical;
        // Universe: SubUni (low), Net (high)
        buf[14] = self.sub_uni;
        buf[15] = self.net;
        // Length (big-endian)
        buf[16..18].copy_from_slice(&self.length.to_be_bytes());
        // DMX data
        buf[18..18 + length].copy_from_slice(&self.data[..length]);

        Ok(total)
    }
}
