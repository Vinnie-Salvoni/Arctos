use core::{
    mem::size_of,
    ptr::{copy_nonoverlapping, read_volatile, write_volatile},
    sync::atomic::{fence, Ordering},
};

use crate::{
    net::driver::{DriverError, NetworkDriver, NetworkInfo},
    pci::PciDevice,
};

const REGISTER_MAP_SIZE: usize = 0x6000;
const RXO: usize = 0x400C;
const MDIC: usize = 0x0020;

const MDIC_OP_WRITE: u32 = 1 << 26;
const MDIC_OP_READ: u32 = 2 << 26;
const MDIC_PHY_ADDR: u32 = 1 << 21;
const MDIC_READY: u32 = 1 << 28;
const MDIC_ERROR: u32 = 1 << 30;

const PHY_BMCR: u8 = 0;
const PHY_BMCR_LOOPBACK: u16 = 1 << 14;
const PHY_BMCR_DEFAULT: u16 = 0x1140;
const MDIC_WAIT_LIMIT: usize = 100_000;
const CTRL: usize = 0x0000;
const STATUS: usize = 0x0008;
const TCTL: usize = 0x0400;
const TIPG: usize = 0x0410;
const RAL: usize = 0x5400;
const RAH: usize = 0x5404;

const TDBAL: usize = 0x3800;
const TDBAH: usize = 0x3804;
const TDLEN: usize = 0x3808;
const TDH: usize = 0x3810;
const TDT: usize = 0x3818;

const RCTL: usize = 0x0100;
const RDBAL: usize = 0x2800;
const RDBAH: usize = 0x2804;
const RDLEN: usize = 0x2808;
const RDH: usize = 0x2810;
const RDT: usize = 0x2818;

const RCTL_EN: u32 = 1 << 1;
const RCTL_BAM: u32 = 1 << 15;
const RCTL_LBM_MAC: u32 = 1 << 6;
const RX_DESCRIPTOR_COUNT: usize = 64;
const RX_BUFFER_SIZE: usize = 2048;
const RX_STATUS_DD: u8 = 1 << 0;
const RX_STATUS_EOP: u8 = 1 << 1;

const CTRL_RST: u32 = 1 << 26;
const CTRL_FD: u32 = 1 << 0;
const CTRL_SLU: u32 = 1 << 6;
const CTRL_SPEED_MASK: u32 = 0b11 << 8;
const CTRL_FRCSPD: u32 = 1 << 11;
const CTRL_FRCDPX: u32 = 1 << 12;
const STATUS_LU: u32 = 1 << 1;

const TCTL_EN: u32 = 1 << 1;
const TCTL_PSP: u32 = 1 << 3;
const TCTL_CT: u32 = 0x10 << 4;
const TCTL_COLD: u32 = 0x40 << 12;

const TX_CMD_EOP: u8 = 1 << 0;
const TX_CMD_IFCS: u8 = 1 << 1;
const TX_CMD_RS: u8 = 1 << 3;
const TX_STATUS_DD: u8 = 1 << 0;

const TX_DESCRIPTOR_COUNT: usize = 256;
const DMA_PAGE_SIZE: usize = 4096;
const MAX_ETHERNET_FRAME: usize = 1518;
const TX_WAIT_LIMIT: usize = 5_000_000;

const MPC: usize = 0x4010;
const RNBC: usize = 0x40A0;
const GPRC: usize = 0x4074;

const GPTC: usize = 0x4080;
const TPT: usize = 0x40D4;
const BPTC: usize = 0x40F4;

#[repr(C)]
#[derive(Clone, Copy)]
struct TxDescriptor {
    address: u64,
    length: u16,
    checksum_offset: u8,
    command: u8,
    status: u8,
    checksum_start: u8,
    special: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RxDescriptor {
    address: u64,
    length: u16,
    checksum: u16,
    status: u8,
    errors: u8,
    special: u16,
}

const _: () = assert!(size_of::<RxDescriptor>() == 16);

const _: () = assert!(size_of::<TxDescriptor>() == 16);
const _: () = assert!(TX_DESCRIPTOR_COUNT * size_of::<TxDescriptor>() == DMA_PAGE_SIZE);

pub struct E1000Driver {
    device: PciDevice,
    physical_bar0: u64,
    registers: Option<u64>,
    tx_ring_virtual: Option<u64>,
    tx_ring_physical: u64,
    tx_buffer_virtual: Option<u64>,
    tx_buffer_physical: u64,
    tx_tail: usize,
    rx_ring_virtual: Option<u64>,
    rx_buffer_virtual: [u64; RX_DESCRIPTOR_COUNT],
    rx_head: usize,
    mac_address: [u8; 6],
    initialized: bool,
}

impl E1000Driver {
    pub fn new(device: PciDevice, physical_bar0: u64) -> Self {
        Self {
            device,
            physical_bar0,
            registers: None,
            tx_ring_virtual: None,
            tx_ring_physical: 0,
            tx_buffer_virtual: None,
            tx_buffer_physical: 0,
            tx_tail: 0,
            mac_address: [0; 6],
            initialized: false,
            rx_ring_virtual: None,
            rx_buffer_virtual: [0; RX_DESCRIPTOR_COUNT],
            rx_head: 0,
        }
    }

    pub fn phy_debug_status(&self) -> Result<u16, DriverError> {
        if !self.initialized {
            return Err(DriverError::NotInitialized);
        }

        self.read_phy_register(PHY_BMCR)
    }

    fn read_reg(&self, offset: usize) -> u32 {
        let base = self.registers.expect("E1000 registers not mapped");
        unsafe { read_volatile((base + offset as u64) as *const u32) }
    }

    fn write_reg(&self, offset: usize, value: u32) {
        let base = self.registers.expect("E1000 registers not mapped");
        unsafe {
            write_volatile((base + offset as u64) as *mut u32, value);
        }
    }

    fn read_mac_from_registers(&self) -> Option<[u8; 6]> {
        let low = self.read_reg(RAL);
        let high = self.read_reg(RAH);

        let mac = [
            low as u8,
            (low >> 8) as u8,
            (low >> 16) as u8,
            (low >> 24) as u8,
            high as u8,
            (high >> 8) as u8,
        ];

        if mac.iter().all(|byte| *byte == 0) || mac.iter().all(|byte| *byte == 0xFF) {
            return None;
        }

        Some(mac)
    }

    fn initialise_tx(&mut self) -> Result<(), DriverError> {
        let (ring_virtual, ring_physical) =
            crate::memory_mapper::allocate_dma_page().ok_or(DriverError::DeviceUnavailable)?;

        let (buffer_virtual, buffer_physical) =
            crate::memory_mapper::allocate_dma_page().ok_or(DriverError::DeviceUnavailable)?;

        if ring_physical & 0xFFF != 0 || buffer_physical & 0xFFF != 0 {
            return Err(DriverError::DeviceUnavailable);
        }

        let ring = ring_virtual as *mut TxDescriptor;

        // Every descriptor references the dedicated packet buffer.
        // Only one transmission is submitted at a time.
        for index in 0..TX_DESCRIPTOR_COUNT {
            unsafe {
                write_volatile(
                    ring.add(index),
                    TxDescriptor {
                        address: buffer_physical,
                        length: 0,
                        checksum_offset: 0,
                        command: 0,
                        status: TX_STATUS_DD,
                        checksum_start: 0,
                        special: 0,
                    },
                );
            }
        }

        self.tx_ring_virtual = Some(ring_virtual);
        self.tx_ring_physical = ring_physical;
        self.tx_buffer_virtual = Some(buffer_virtual);
        self.tx_buffer_physical = buffer_physical;
        self.tx_tail = 0;

        // Disable TX while programming its descriptor ring.
        self.write_reg(TCTL, 0);

        self.write_reg(TDBAL, ring_physical as u32);
        self.write_reg(TDBAH, (ring_physical >> 32) as u32);
        self.write_reg(TDLEN, (TX_DESCRIPTOR_COUNT * 16) as u32);
        self.write_reg(TDH, 0);
        self.write_reg(TDT, 0);

        // E1000 recommended transmit inter-packet gap.
        self.write_reg(TIPG, 0x0060_200A);

        // Enable transmission, pad short frames and set collision parameters.
        self.write_reg(TCTL, TCTL_EN | TCTL_PSP | TCTL_CT | TCTL_COLD);

        fence(Ordering::SeqCst);
        Ok(())
    }

    fn initialise_rx(&mut self) -> Result<(), DriverError> {
        let (ring_virtual, ring_physical) =
            crate::memory_mapper::allocate_dma_page().ok_or(DriverError::DeviceUnavailable)?;

        if ring_physical & 0xFFF != 0 {
            return Err(DriverError::DeviceUnavailable);
        }

        let ring = ring_virtual as *mut RxDescriptor;

        for index in 0..RX_DESCRIPTOR_COUNT {
            let (buffer_virtual, buffer_physical) =
                crate::memory_mapper::allocate_dma_page().ok_or(DriverError::DeviceUnavailable)?;

            if buffer_physical & 0xFFF != 0 {
                return Err(DriverError::DeviceUnavailable);
            }

            self.rx_buffer_virtual[index] = buffer_virtual;

            unsafe {
                write_volatile(
                    ring.add(index),
                    RxDescriptor {
                        address: buffer_physical,
                        length: 0,
                        checksum: 0,
                        status: 0,
                        errors: 0,
                        special: 0,
                    },
                );
            }
        }
        let first_rx_buffer =
            unsafe { core::ptr::read_volatile(core::ptr::addr_of!((*ring).address)) };
        self.rx_ring_virtual = Some(ring_virtual);
        self.rx_head = 0;

        // Disable RX while programming the receive descriptor ring.
        self.write_reg(RCTL, 0);

        self.write_reg(RDBAL, ring_physical as u32);
        self.write_reg(RDBAH, (ring_physical >> 32) as u32);
        self.write_reg(
            RDLEN,
            (RX_DESCRIPTOR_COUNT * size_of::<RxDescriptor>()) as u32,
        );
        self.write_reg(RDH, 0);
        self.write_reg(RDT, (RX_DESCRIPTOR_COUNT - 1) as u32);

        // Enable receive with an explicit 2048-byte buffer size.
        // BSIZE=00 selects 2048 bytes; BSEX remains clear.
        // Also accept broadcast frames.
        const RCTL_BSIZE_MASK: u32 = 0b11 << 16;
        const RCTL_BSEX: u32 = 1 << 25;

        let receive_control = RCTL_EN | RCTL_BAM;

        // Make the buffer-size configuration explicit rather than relying
        // on any previous receive-control register state.
        let receive_control = receive_control & !(RCTL_BSIZE_MASK | RCTL_BSEX);

        fence(Ordering::SeqCst);
        self.write_reg(RCTL, receive_control);
        fence(Ordering::SeqCst);

        Ok(())
    }

    fn transmit_frame(&mut self, frame: &[u8]) -> Result<(), DriverError> {
        if !self.initialized {
            return Err(DriverError::NotInitialized);
        }

        if frame.len() < 14 || frame.len() > MAX_ETHERNET_FRAME {
            return Err(DriverError::BufferTooSmall);
        }

        let ring_virtual = self.tx_ring_virtual.ok_or(DriverError::NotInitialized)?;
        let buffer_virtual = self.tx_buffer_virtual.ok_or(DriverError::NotInitialized)?;

        let descriptor = (ring_virtual as *mut TxDescriptor).wrapping_add(self.tx_tail);

        // Wait for the previous use of this descriptor to finish.
        let mut ready = false;

        for _ in 0..TX_WAIT_LIMIT {
            let status = unsafe { read_volatile(core::ptr::addr_of!((*descriptor).status)) };

            if status & TX_STATUS_DD != 0 {
                ready = true;
                break;
            }

            core::hint::spin_loop();
        }

        if !ready {
            return Err(DriverError::TransmitFailed);
        }

        // Copy the frame into DMA-accessible RAM.
        unsafe {
            copy_nonoverlapping(frame.as_ptr(), buffer_virtual as *mut u8, frame.len());
            let tx_bytes = buffer_virtual as *const u8;

            crate::println!(
                "[NET] DMA TX destination: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                read_volatile(tx_bytes.add(0)),
                read_volatile(tx_bytes.add(1)),
                read_volatile(tx_bytes.add(2)),
                read_volatile(tx_bytes.add(3)),
                read_volatile(tx_bytes.add(4)),
                read_volatile(tx_bytes.add(5)),
            );

            crate::println!(
                "[NET] DMA TX source: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                read_volatile(tx_bytes.add(6)),
                read_volatile(tx_bytes.add(7)),
                read_volatile(tx_bytes.add(8)),
                read_volatile(tx_bytes.add(9)),
                read_volatile(tx_bytes.add(10)),
                read_volatile(tx_bytes.add(11)),
            );

            crate::println!(
                "[NET] DMA TX EtherType: {:02X}:{:02X}",
                read_volatile(tx_bytes.add(12)),
                read_volatile(tx_bytes.add(13)),
            );
            write_volatile(
                core::ptr::addr_of_mut!((*descriptor).address),
                self.tx_buffer_physical,
            );
            write_volatile(
                core::ptr::addr_of_mut!((*descriptor).length),
                frame.len() as u16,
            );
            write_volatile(core::ptr::addr_of_mut!((*descriptor).checksum_offset), 0);
            write_volatile(core::ptr::addr_of_mut!((*descriptor).checksum_start), 0);
            write_volatile(core::ptr::addr_of_mut!((*descriptor).special), 0);
            write_volatile(core::ptr::addr_of_mut!((*descriptor).status), 0);
            write_volatile(
                core::ptr::addr_of_mut!((*descriptor).command),
                TX_CMD_EOP | TX_CMD_IFCS | TX_CMD_RS,
            );
        }

        // Make the descriptor and packet visible before notifying the NIC.
        fence(Ordering::SeqCst);

        let next_tail = (self.tx_tail + 1) % TX_DESCRIPTOR_COUNT;
        self.write_reg(TDT, next_tail as u32);

        // Wait until the device reports descriptor completion.
        let mut completed = false;

        for _ in 0..TX_WAIT_LIMIT {
            let status = unsafe { read_volatile(core::ptr::addr_of!((*descriptor).status)) };

            if status & TX_STATUS_DD != 0 {
                completed = true;
                break;
            }

            core::hint::spin_loop();
        }

        if !completed {
            return Err(DriverError::TransmitFailed);
        }

        self.tx_tail = next_tail;
        Ok(())
    }

    fn read_phy_register(&self, register: u8) -> Result<u16, DriverError> {
        let command = MDIC_OP_READ | MDIC_PHY_ADDR | ((register as u32) << 16);

        self.write_reg(MDIC, command);

        for _ in 0..MDIC_WAIT_LIMIT {
            let result = self.read_reg(MDIC);

            if result & MDIC_READY != 0 {
                if result & MDIC_ERROR != 0 {
                    return Err(DriverError::DeviceUnavailable);
                }

                return Ok(result as u16);
            }

            core::hint::spin_loop();
        }

        Err(DriverError::DeviceUnavailable)
    }

    fn write_phy_register(&self, register: u8, value: u16) -> Result<(), DriverError> {
        let command = MDIC_OP_WRITE | MDIC_PHY_ADDR | ((register as u32) << 16) | value as u32;

        self.write_reg(MDIC, command);

        for _ in 0..MDIC_WAIT_LIMIT {
            let result = self.read_reg(MDIC);

            if result & MDIC_READY != 0 {
                if result & MDIC_ERROR != 0 {
                    return Err(DriverError::DeviceUnavailable);
                }

                return Ok(());
            }

            core::hint::spin_loop();
        }

        Err(DriverError::DeviceUnavailable)
    }

    /// Enable MAC loopback for a local transmit/receive test.
    ///
    /// This changes the receive configuration only; normal operation can be
    /// restored by calling `disable_loopback`.

    pub fn enable_loopback(&mut self) -> Result<(), DriverError> {
        if !self.initialized {
            return Err(DriverError::NotInitialized);
        }

        // Stop receive while changing loopback configuration.
        self.write_reg(RCTL, 0);
        fence(Ordering::SeqCst);

        // QEMU's E1000 model loops transmitted packets back through
        // its receive path when the PHY BMCR loopback bit is set.
        self.write_phy_register(PHY_BMCR, PHY_BMCR_DEFAULT | PHY_BMCR_LOOPBACK)?;

        // Accept broadcast frames and enable receive.
        const RCTL_UPE: u32 = 1 << 3;

        self.write_reg(RCTL, RCTL_EN | RCTL_BAM | RCTL_UPE);
        fence(Ordering::SeqCst);

        crate::println!("[NET] CTRL={:#010X}", self.read_reg(CTRL));
        crate::println!(
            "[NET] STATUS={:#010X} (link-up bit: {})",
            self.read_reg(STATUS),
            if self.read_reg(STATUS) & STATUS_LU != 0 {
                "SET"
            } else {
                "CLEAR"
            }
        );
        crate::println!(
            "[NET] PHY BMCR={:#06X}, RCTL={:#010X}",
            self.read_phy_register(PHY_BMCR)?,
            self.read_reg(RCTL)
        );

        Ok(())
    }

    /// Capture RX registers and every descriptor status without consuming frames.

    pub fn rx_debug_snapshot(
        &self,
    ) -> Result<([u8; RX_DESCRIPTOR_COUNT], u32, u32, u32, u32, u32, u32, u64), DriverError> {
        if !self.initialized {
            return Err(DriverError::NotInitialized);
        }

        let ring_virtual = self.rx_ring_virtual.ok_or(DriverError::NotInitialized)?;

        fence(Ordering::SeqCst);

        let mut statuses = [0u8; RX_DESCRIPTOR_COUNT];
        let ring = ring_virtual as *const RxDescriptor;

        for (index, status) in statuses.iter_mut().enumerate() {
            let descriptor = unsafe { ring.add(index) };
            *status = unsafe { read_volatile(core::ptr::addr_of!((*descriptor).status)) };
        }

        let first_rx_buffer = unsafe { read_volatile(core::ptr::addr_of!((*ring).address)) };

        let rctl = self.read_reg(RCTL);
        let rdh = self.read_reg(RDH);
        let rdt = self.read_reg(RDT);
        let rdlen = self.read_reg(RDLEN);
        let rdbal = self.read_reg(RDBAL);
        let rdbah = self.read_reg(RDBAH);
        crate::println!(
            "[NET] TX counters: GPTC={} TPT={} BPTC={}",
            self.read_reg(GPTC),
            self.read_reg(TPT),
            self.read_reg(BPTC)
        );
        crate::println!(
            "[NET] RX counters: MPC={} RNBC={} GPRC={}",
            self.read_reg(MPC),
            self.read_reg(RNBC),
            self.read_reg(GPRC)
        );
        crate::println!("[NET] RX overflow: RXO={}", self.read_reg(RXO));
        Ok((
            statuses,
            rctl,
            rdh,
            rdt,
            rdlen,
            rdbal,
            rdbah,
            first_rx_buffer,
        ))
    }

    pub fn tx_debug_snapshot(&self) -> Result<(u32, u32, u32, u32, u8), DriverError> {
        if !self.initialized {
            return Err(DriverError::NotInitialized);
        }

        let ring_virtual = self.tx_ring_virtual.ok_or(DriverError::NotInitialized)?;

        let descriptor_index = if self.tx_tail == 0 {
            TX_DESCRIPTOR_COUNT - 1
        } else {
            self.tx_tail - 1
        };

        let descriptor = unsafe { (ring_virtual as *const TxDescriptor).add(descriptor_index) };

        fence(Ordering::SeqCst);

        let status = unsafe { read_volatile(core::ptr::addr_of!((*descriptor).status)) };

        Ok((
            self.read_reg(TDH),
            self.read_reg(TDT),
            self.read_reg(TDLEN),
            self.read_reg(TCTL),
            status,
        ))
    }

    /// Restore normal receive mode after a loopback test.

    pub fn disable_loopback(&mut self) -> Result<(), DriverError> {
        if !self.initialized {
            return Err(DriverError::NotInitialized);
        }

        self.write_reg(RCTL, 0);
        fence(Ordering::SeqCst);

        self.write_phy_register(PHY_BMCR, PHY_BMCR_DEFAULT)?;

        self.write_reg(RCTL, RCTL_EN | RCTL_BAM);
        fence(Ordering::SeqCst);

        Ok(())
    }
}

impl NetworkDriver for E1000Driver {
    fn init(&mut self) -> Result<(), DriverError> {
        if self.initialized {
            return Ok(());
        }

        // Never bind this implementation to an unrelated network controller.
        if self.device.vendor_id != 0x8086 || self.device.device_id != 0x100E {
            return Err(DriverError::DeviceUnavailable);
        }

        if !crate::pci::enable_memory_and_bus_mastering(&self.device) {
            return Err(DriverError::DeviceUnavailable);
        }

        let registers =
            unsafe { crate::memory_mapper::map_mmio(self.physical_bar0, REGISTER_MAP_SIZE) }
                .ok_or(DriverError::DeviceUnavailable)?;

        self.registers = Some(registers);

        // Reset the adapter and wait for the reset bit to clear.
        let control = self.read_reg(CTRL);
        self.write_reg(CTRL, control | CTRL_RST);
        fence(Ordering::SeqCst);

        let mut reset_complete = false;

        for _ in 0..TX_WAIT_LIMIT {
            if self.read_reg(CTRL) & CTRL_RST == 0 {
                reset_complete = true;
                break;
            }

            core::hint::spin_loop();
        }

        if !reset_complete {
            return Err(DriverError::DeviceUnavailable);
        }
        // Request link-up and full-duplex operation after reset.
        let control = self.read_reg(CTRL);
        self.write_reg(CTRL, control | CTRL_SLU | CTRL_FD);
        fence(Ordering::SeqCst);

        crate::println!(
            "[NET] CTRL={:#010X}, STATUS={:#010X}",
            self.read_reg(CTRL),
            self.read_reg(STATUS)
        );
        self.mac_address = self
            .read_mac_from_registers()
            .ok_or(DriverError::DeviceUnavailable)?;

        self.initialise_tx()?;
        self.initialise_rx()?;
        self.initialized = true;

        Ok(())
    }

    fn info(&self) -> NetworkInfo {
        let link_up = self.initialized && (self.read_reg(STATUS) & STATUS_LU != 0);

        NetworkInfo {
            mac_address: self.mac_address,
            link_up,
        }
    }

    fn transmit(&mut self, frame: &[u8]) -> Result<(), DriverError> {
        self.transmit_frame(frame)
    }

    fn receive(&mut self, buffer: &mut [u8]) -> Result<Option<usize>, DriverError> {
        if !self.initialized {
            return Err(DriverError::NotInitialized);
        }

        let ring_virtual = self.rx_ring_virtual.ok_or(DriverError::NotInitialized)?;
        let descriptor = (ring_virtual as *mut RxDescriptor).wrapping_add(self.rx_head);

        let status = unsafe { read_volatile(core::ptr::addr_of!((*descriptor).status)) };

        // No packet has arrived in this descriptor yet.
        if status & RX_STATUS_DD == 0 {
            return Ok(None);
        }

        fence(Ordering::Acquire);

        let length = unsafe { read_volatile(core::ptr::addr_of!((*descriptor).length)) as usize };
        let errors = unsafe { read_volatile(core::ptr::addr_of!((*descriptor).errors)) };

        let result = if errors != 0 || status & RX_STATUS_EOP == 0 {
            Err(DriverError::DeviceUnavailable)
        } else if length == 0 || length > RX_BUFFER_SIZE {
            Err(DriverError::DeviceUnavailable)
        } else if buffer.len() < length {
            Err(DriverError::BufferTooSmall)
        } else {
            let source = self.rx_buffer_virtual[self.rx_head] as *const u8;

            unsafe {
                copy_nonoverlapping(source, buffer.as_mut_ptr(), length);
            }

            Ok(Some(length))
        };

        // Return this descriptor to the NIC, even if the packet was invalid
        // or the caller's output buffer was too small.
        unsafe {
            write_volatile(core::ptr::addr_of_mut!((*descriptor).length), 0);
            write_volatile(core::ptr::addr_of_mut!((*descriptor).checksum), 0);
            write_volatile(core::ptr::addr_of_mut!((*descriptor).errors), 0);
            write_volatile(core::ptr::addr_of_mut!((*descriptor).special), 0);
            write_volatile(core::ptr::addr_of_mut!((*descriptor).status), 0);
        }

        fence(Ordering::SeqCst);

        self.write_reg(RDT, self.rx_head as u32);
        self.rx_head = (self.rx_head + 1) % RX_DESCRIPTOR_COUNT;

        result
    }
}

/// Read the adapter's MAC address without initialising transmission.
///
/// # Safety
/// `physical_bar0` must be the verified physical MMIO BAR0 of a compatible
/// E1000 device, and the reserved MMIO mapping must be safe to use.
pub unsafe fn read_mac(physical_bar0: u64) -> Option<[u8; 6]> {
    let base = crate::memory_mapper::map_mmio(physical_bar0, REGISTER_MAP_SIZE)?;

    let low = read_volatile((base + RAL as u64) as *const u32);
    let high = read_volatile((base + RAH as u64) as *const u32);

    let mac = [
        low as u8,
        (low >> 8) as u8,
        (low >> 16) as u8,
        (low >> 24) as u8,
        high as u8,
        (high >> 8) as u8,
    ];

    if mac.iter().all(|byte| *byte == 0) || mac.iter().all(|byte| *byte == 0xFF) {
        return None;
    }

    Some(mac)
}
