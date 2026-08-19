use bitflags::bitflags;

use super::ArtNetError;

const ART_POLL_REPLY_OPCODE: u16 = 0x2100;

pub const BUFFER_SIZE: usize = 240; // Standard ArtPollReply size

#[derive(Clone)]
pub struct PollReply {
    /// Array containing the Node’s IP address. First
    /// array entry is most significant byte of address.
    /// When binding is implemented, bound nodes may
    /// share the root node’s IP Address and the
    /// BindIndex is used to differentiate the nodes.
    pub ip_address: [u8; 4],
    /// The Port is always 0x1936
    pub port: u16,
    /// The Firmware Vesion of the Node.
    pub firmware_version: u16,

    /// The Net that the Node is set to. (7 bit value)
    pub net: u8,
    /// The Sub-Net that the Node is set to. (4 bit value)
    pub sub_net: u8,

    /// The OEM Code
    pub oem_code: u16,
    /// This field contains the firmware version of the
    /// User Bios Extension Area (UBEA). If the UBEA is
    /// not programmed, this field contains zero.
    pub ubea_version: u8,
    /// General Status register containing bit fields.
    pub status1: Status1,
    /// ESTA manufacturer code.
    pub esta_manufacturer: u16,
    /// Null terminated name for each port of the node.
    pub port_name: [u8; 18],
    /// Null terminated long name for the device.
    pub long_name: [u8; 64],
    /// The array is a textual report of the Node’s
    /// operating status or operational errors. It is
    /// primarily intended for ‘engineering’ data rather
    /// than ‘end user’ data. The field is formatted as:
    /// “#xxxx [yyyy..] zzzzz…”
    /// xxxx is a hex status code as defined in Table 3.
    /// yyyy is a decimal counter that increments every
    /// time the Node sends an ArtPollResponse.
    /// This allows the controller to monitor event
    /// changes in the Node.
    pub node_report: [u8; 64],
    /// Number of inout or output ports. Max 4.
    pub num_ports: u16,
    /// This array defines the operation and protocol of
    /// each channel. (A product with 4 inputs and 4
    ///     outputs would report 0xc0, 0xc0, 0xc0, 0xc0). The
    ///     array length is fixed, independent of the number
    ///     of inputs or outputs physically available on the
    ///    Node.
    pub port_types: [PortTypes; 4],
    /// This array defined input status of the node
    pub good_input: [GoodInput; 4],
    /// This array defined output status of the node
    pub good_output: [GoodOutputA; 4],
    /// Bits 3-0 of the 15 bit Port-Address of each input port
    pub sw_in: [u8; 4],
    /// Bits 3-0 of the 15 bit Port-Address of each output port
    pub sw_out: [u8; 4],
    /// The sACN priority value that will be used when any received DM is converted to sACN.
    pub acn_priority: u8,
    /// Macro trigger values
    pub sw_macro: SwMacro,
    /// Remote trigger values
    pub sw_remote: SwRemote,

    //3 spare bytes
    /// Style code
    pub style: StyleCode,
    /// MAC Address
    pub mac: [u8; 6],
    /// If this unit is part of a larger or modular product, this is the IP of the root device.
    pub bind_ip: [u8; 4],
    /// This number represents the order of bound devices. A lower number means closer to root device.
    pub bind_index: u8,
    /// Status2
    pub status2: Status2,
    /// GoodOutputB
    pub good_output_b: [GoodOutputB; 4],
    /// Status3
    pub status3: Status3,
    /// RDMnet & LLRP Default responder UID
    pub uid: [u8; 6],
    /// Available for user specific data
    pub user_data: [u8; 2],
    /// RefreshRate
    pub refresh_rate: u16,
    // 11 bytes of filler
}

impl PollReply {
    /// Write the PollReply message to a buffer.
    /// Returns the number of bytes written on success, or an error if buffer is too small.
    pub fn to_buffer(&self, buf: &mut [u8]) -> Result<usize, ArtNetError> {
        if buf.len() < BUFFER_SIZE {
            return Err(ArtNetError::BufferTooSmall);
        }

        buf[0..8].copy_from_slice(b"Art-Net\0");
        buf[8] = 0x00;
        buf[9] = 0x21;
        buf[10] = self.ip_address[0];
        buf[11] = self.ip_address[1];
        buf[12] = self.ip_address[2];
        buf[13] = self.ip_address[3];
        
        buf[14] = self.port as u8;
        buf[15] = (self.port >> 8) as u8;

        buf[16] = (self.firmware_version >> 8) as u8;
        buf[17] = self.firmware_version as u8;
        buf[18] = self.net;
        buf[19] = self.sub_net;
        buf[20] = (self.oem_code >> 8) as u8;
        buf[21] = self.oem_code as u8;
        buf[22] = self.ubea_version;
        buf[23] = self.status1.bits();
        buf[24] = self.esta_manufacturer as u8;
        buf[25] = (self.esta_manufacturer >> 8) as u8;
        buf[26..44].copy_from_slice(&self.port_name);
        buf[44..108].copy_from_slice(&self.long_name);
        buf[108..172].copy_from_slice(&self.node_report);
        buf[172] = (self.num_ports >> 8) as u8;
        buf[173] = self.num_ports as u8;

        buf[175] = self.port_types[0].bits();
        buf[176] = self.port_types[1].bits();
        buf[177] = self.port_types[2].bits();
        buf[178] = self.port_types[3].bits();

        buf[179] = self.good_input[0].bits();
        buf[180] = self.good_input[1].bits();
        buf[181] = self.good_input[2].bits();
        buf[182] = self.good_input[3].bits();

        buf[183] = self.good_output[0].bits();
        buf[184] = self.good_output[1].bits();
        buf[185] = self.good_output[2].bits();
        buf[186] = self.good_output[3].bits();

        buf[187] = self.sw_in[0];
        buf[188] = self.sw_in[1];
        buf[189] = self.sw_in[2];
        buf[190] = self.sw_in[3];

        buf[191] = self.sw_out[0];
        buf[192] = self.sw_out[1];
        buf[193] = self.sw_out[2];
        buf[194] = self.sw_out[3];

        buf[195] = self.acn_priority;
        buf[196] = self.sw_macro.bits();
        buf[197] = self.sw_remote.bits();
        buf[201] = self.style as u8;
        buf[202..208].copy_from_slice(&self.mac);
        buf[208..212].copy_from_slice(&self.bind_ip);
        buf[212] = self.bind_index;
        buf[213] = self.status2.bits();

        buf[214] = self.good_output_b[0].bits();
        buf[215] = self.good_output_b[1].bits();
        buf[216] = self.good_output_b[2].bits();
        buf[217] = self.good_output_b[3].bits();

        buf[218] = self.status3.bits();
        buf[219..225].copy_from_slice(&self.uid);
        buf[225..227].copy_from_slice(&self.user_data);
        buf[227] = (self.refresh_rate >> 8) as u8;
        buf[228] = self.refresh_rate as u8;
        buf[229..240].fill(0);
        Ok(BUFFER_SIZE)
    }
}

bitflags! {
    #[derive(Clone, Copy)]
    pub struct Status1: u8 {
        /// 1 = UBEA present
        const UBEA = 0b00000001;
        /// 1 = RDM Capable
        const RDM = 0b00000010;
        /// 1 = Booted from ROM
        const ROM = 0b00000100;

        // Port Address Programming Authority

        /// 01 = All Port-Address set by front panel controls.
        const PortAddressFrontPanel = 0b00010000;
        /// 10 = All Port-Address set by network data.
        const PortAddressNetwork = 0b00100000;

        // Indicator state
        /// Indicators in Locate / Identify mode.
        const IndicatorLocate = 0b01000000;
        /// Indicators in Mute mode.
        const IndicatorMute = 0b10000000;
        /// Indicators in Normal mode.
        const IndicatorNormal = 0b11000000;
    }
}

bitflags! {

    #[derive(Clone, Copy)]
    pub struct PortTypes: u8 {
        // Lower 5 bits are the port type.
        const DMX512 = 0b00000000;
        const MIDI = 0b00000001;
        const Avab = 0b00000010;
        const ColortranCmx = 0b00000011;
        const ADB625 = 0b00000100;
        const ArtNet = 0b00000101;
        const Dali = 0b00000110;

        /// 1 = Input onto the network.
        const Input = 0b01000000;
        /// 1 = Output from the network.
        const Output = 0b10000000;
    }
}

bitflags! {
    #[derive(Clone, Copy)]
    pub struct GoodInput: u8 {
        /// 1 = Data received.
        const DataReceived = 0b10000000;
        /// 1 = Channel includes DMX512 test packets.
        const DMX512Test = 0b01000000;
        /// 1 = Channel includes DMX512 SIP’s.
        const DMX512SIP = 0b00100000;
        /// 1 = Channel includes DMX512 text packets.
        const DMX512Text = 0b00010000;
        /// 1 = Input is disabled.
        const InputDisabled = 0b00001000;
        /// 1 = Receive errors detected.
        const ReceiveErrors = 0b00000100;
        /// 1 = Input is selected to convert to sACN.
        /// 0  = Input is selected to convert to Art-Net.
        const SACN = 0b00000001;

    }
}

bitflags! {
    #[derive(Debug, Default, Clone, Copy)]
    pub struct GoodOutputA: u8 {
        /// 1 = Data is being transmitted.
        const DataTransmitting = 0b10000000;
        /// 1 = Channel includes DMX512 test packets.
        const DMX512Test = 0b01000000;
        /// 1 = Channel includes DMX512 SIP’s.
        const DMX512SIP = 0b00100000;
        /// 1 = Channel includes DMX512 text packets.
        const DMX512Text = 0b00010000;
        /// 1 = Output is merging Art-Net data.
        const OutputMerging = 0b00001000;
        /// 1 = DMX output short detected.
        const TransmitErrors = 0b00000100;
        /// 1 = Merge Mode is LTP
        const MergeLTP = 0b00000010;
        /// 1 = Output is selected to convert from sACN.
        /// 0  = Output is selected to convert from Art-Net.
        const SACN = 0b00000001;
    }
}

bitflags! {
    #[derive(Debug, Default, Clone, Copy)]
    pub struct SwMacro: u8 {
        /// 1 = Macro 8 active
        const Macro8 = 0b10000000;
        /// 1 = Macro 7 active
        const Macro7 = 0b01000000;
        /// 1 = Macro 6 active
        const Macro6 = 0b00100000;
        /// 1 = Macro 5 active
        const Macro5 = 0b00010000;
        /// 1 = Macro 4 active
        const Macro4 = 0b00001000;
        /// 1 = Macro 3 active
        const Macro3 = 0b00000100;
        /// 1 = Macro 2 active
        const Macro2 = 0b00000010;
        /// 1 = Macro 1 active
        const Macro1 = 0b00000001;
    }
}

bitflags! {
    #[derive(Debug, Default, Clone, Copy)]
    pub struct SwRemote: u8 {
        /// 1 = Remote 8 active
        const Remote8 = 0b10000000;
        /// 1 = Remote 7 active
        const Remote7 = 0b01000000;
        /// 1 = Remote 6 active
        const Remote6 = 0b00100000;
        /// 1 = Remote 5 active
        const Remote5 = 0b00010000;
        /// 1 = Remote 4 active
        const Remote4 = 0b00001000;
        /// 1 = Remote 3 active
        const Remote3 = 0b00000100;
        /// 1 = Remote 2 active
        const Remote2 = 0b00000010;
        /// 1 = Remote 1 active
        const Remote1 = 0b00000001;
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum StyleCode {
    Node = 0x00,
    Controller = 0x01,
    Media = 0x02,
    Route = 0x03,
    Backup = 0x04,
    Config = 0x05,
    Visual = 0x06,
}

bitflags! {
    #[derive(Debug, Default, Clone, Copy)]
    pub struct Status2: u8 {
        /// 1 Node supports control of RDM using ArtCommand
        const RDM = 0b10000000;
        /// 1 = Node supports switching of output style using ArtCommand
        const OutputStyle = 0b01000000;
        /// 1 = Squawking
        const Squawking = 0b00100000;
        /// 1 = Node is able to switch between Art-Net and sACN
        const SACN_Switch = 0b00010000;
        /// 0 = Node suports 8-bit Port Adresses
        /// 1 = Node supports 15-bit Port Addresses
        const PortAddress15Bit = 0b00001000;
        /// 1 = Node is DHCP capable
        const DHCP = 0b00000100;
        /// 1 = IP is DHCP configured
        const DHCP_Configured = 0b00000010;
        /// 1 = Node supports web browser configuration
        const WebConfig = 0b00000001;
    }
}

bitflags! {
    #[derive(Debug, Default, Clone, Copy)]
    pub struct GoodOutputB: u8 {
        /// 1 = RDM enabled
        const RDM = 0b10000000;
        /// 1 = Output style is continuous
        /// 0 = Output style is delta
        const OutputStyleContinuous = 0b01000000;
        /// 1 = Discovery is currently not running
        const DiscoveryNot = 0b00100000;
        /// 1 = Background discovery disabled.
        const BackgroundDiscoveryDisabled = 0b00010000;
    }
}

bitflags! {
    #[derive(Debug, Default, Clone, Copy)]
    pub struct Status3: u8 {
        // Failsafe state
        /// 00 = hold
        const FailsafeHold = 0b00000000;
        /// 01 = All outputs to zero
        const FailsafeZero = 0b01000000;
        /// 10 = All outputs to full
        const FailsafeFull = 0b10000000;
        /// 11 = Playback fail safe scene
        const FailsafeScene = 0b11000000;

        /// 1 = Node supports fail over
        const FailOver = 0b00100000;
        /// 1 = Node supports LLRP
        const LLRP = 0b00010000;
        /// 1 = Node supports switching ports between input and output
        const PortSwitching = 0b00001000;
        /// 1 = Node supports RDMnet
        const RDMnet = 0b00000100;
    }
}


