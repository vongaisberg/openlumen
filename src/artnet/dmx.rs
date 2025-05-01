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
    pub fn from_buffer(buf: &[u8]) -> Option<Self> {
        if buf.len() < 19 {
            return None;
        }
        let mut res = Self {
            sequence: buf[12],
            physical: buf[13],
            sub_uni: buf[14],
            net: buf[15],
            length: u16::from_be_bytes([buf[16], buf[17]]),
            data: [0; 512],
        };

        res.data.copy_from_slice(&buf[18..18 + res.length as usize]);

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
}
