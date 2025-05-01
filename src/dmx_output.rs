use embassy_stm32::pac::eth::vals::Dm;

pub struct Dmx_Output {
    /// Physical UART buffer
    buf: [u8; 708],
}
impl Dmx_Output {
    pub fn new() -> Self {
        let mut buf = [0; 708];

        Self::set_bit(&mut buf, 22, true);
        Self::set_bit(&mut buf, 23, true);

        // Set stop bits for each data byte
        for i in 0..512 {
            Self::set_bit(&mut buf, 44 + i * 11, false);
            Self::set_bit(&mut buf, 45 + i * 11, false);
        }
        Self { buf }
    }

    fn set_bit(buf: &mut [u8; 708], bit: usize, value: bool) {
        let byte = bit / 8;
        let bit = bit % 8;
        if value {
            buf[byte] |= 1 << bit;
        } else {
            buf[byte] &= !(1 << bit);
        }
    }
    /// Write eight concecutive bits to the buffer
    fn write_byte(&mut self, start_bit: usize, value: u8) {
        // This function can affect two bytes at most
        let start_byte = start_bit / 8;
        let end_byte = (start_bit + 8) / 8;
        let start_bit = start_bit % 8;
        let end_bit = (start_bit + 8) % 8;
        self.buf[start_byte] = (self.buf[start_byte] & !(0xFF >> start_bit)) | (value << start_bit);
        self.buf[end_byte] = (self.buf[end_byte] & !(0xFF << end_bit)) | (value >> (8 - end_bit));
    }

    /// Set the nth data byte
    pub fn set_data_byte(&mut self, index: usize, value: u8) {
        self.write_byte(36 + index * 11, value);
    }

    pub fn set_data_from_artnet(&mut self, data: &[u8; 512]) {
        for i in 0..512 {
            self.set_data_byte(i, data[i]);
        }
    }
}
