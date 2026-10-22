/// Hardware-independent network device interface.
///
/// Individual drivers implement this interface so the networking
/// stack does not need to know which network card is installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverError {
    NotInitialized,
    DeviceUnavailable,
    BufferTooSmall,
    TransmitFailed,
}

/// Common information exposed by a network interface.
#[derive(Debug, Clone, Copy)]
pub struct NetworkInfo {
    pub mac_address: [u8; 6],
    pub link_up: bool,
}

/// Interface every supported network-card driver must implement.
pub trait NetworkDriver {
    /// Initialize the hardware and prepare its transmit/receive queues.
    fn init(&mut self) -> Result<(), DriverError>;

    /// Return the interface's MAC address and link status.
    fn info(&self) -> NetworkInfo;

    /// Transmit one complete Ethernet frame.
    fn transmit(&mut self, frame: &[u8]) -> Result<(), DriverError>;

    /// Receive one Ethernet frame into the supplied buffer.
    ///
    /// Returns the number of bytes received, or `Ok(None)` when
    /// no frame is currently available.
    fn receive(&mut self, buffer: &mut [u8]) -> Result<Option<usize>, DriverError>;
}
