use alloc::vec::Vec;
use x86_64::instructions::port::Port;

const CONFIG_ADDRESS: u16 = 0xCF8;
const CONFIG_DATA: u16 = 0xCFC;

#[derive(Debug, Clone, Copy)]
pub struct PciDevice {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class_code: u8,
    pub subclass: u8,
    pub programming_interface: u8,
    pub header_type: u8,
}

#[derive(Debug, Clone, Copy)]
pub enum PciBar {
    Unused,
    Io { port: u16 },
    Memory32 { address: u32, prefetchable: bool },
    Memory64 { address: u64, prefetchable: bool },
}

unsafe fn read_config(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    let address = 0x8000_0000u32
        | ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((function as u32) << 8)
        | ((offset as u32) & 0xFC);

    let mut address_port = Port::<u32>::new(CONFIG_ADDRESS);
    let mut data_port = Port::<u32>::new(CONFIG_DATA);

    address_port.write(address);
    data_port.read()
}

/// Enable PCI memory-space access and bus mastering for a device.
///
/// Returns true only if both bits are set when read back.
pub fn enable_memory_and_bus_mastering(device: &PciDevice) -> bool {
    let address = 0x8000_0000u32
        | ((device.bus as u32) << 16)
        | ((device.device as u32) << 11)
        | ((device.function as u32) << 8)
        | 0x04;

    // PCI command register is the lower 16 bits at offset 0x04.
    let command =
        unsafe { (read_config(device.bus, device.device, device.function, 0x04) & 0xFFFF) as u16 };

    // Bit 1 = Memory Space Enable; bit 2 = Bus Master Enable.
    let new_command = command | 0x0006;

    unsafe {
        let mut address_port = Port::<u32>::new(CONFIG_ADDRESS);
        let mut data_port = Port::<u16>::new(CONFIG_DATA);

        address_port.write(address);
        data_port.write(new_command);
    }

    let verified =
        unsafe { (read_config(device.bus, device.device, device.function, 0x04) & 0xFFFF) as u16 };

    verified & 0x0006 == 0x0006
}

pub fn scan() -> Vec<PciDevice> {
    let mut devices = Vec::new();

    for bus in 0u16..=255 {
        for device in 0u8..32 {
            let (vendor_device, header) = unsafe {
                (
                    read_config(bus as u8, device, 0, 0),
                    read_config(bus as u8, device, 0, 0x0C),
                )
            };

            if vendor_device as u16 == 0xFFFF {
                continue;
            }

            let header_type = ((header >> 16) & 0xFF) as u8;
            let function_count = if header_type & 0x80 != 0 { 8 } else { 1 };

            for function in 0..function_count {
                let (id, class_register, function_header) = unsafe {
                    (
                        read_config(bus as u8, device, function, 0),
                        read_config(bus as u8, device, function, 0x08),
                        read_config(bus as u8, device, function, 0x0C),
                    )
                };

                let vendor_id = id as u16;

                if vendor_id == 0xFFFF {
                    continue;
                }

                devices.push(PciDevice {
                    bus: bus as u8,
                    device,
                    function,
                    vendor_id,
                    device_id: (id >> 16) as u16,
                    programming_interface: ((class_register >> 8) & 0xFF) as u8,
                    subclass: ((class_register >> 16) & 0xFF) as u8,
                    class_code: ((class_register >> 24) & 0xFF) as u8,
                    header_type: ((function_header >> 16) & 0xFF) as u8,
                });
            }
        }
    }

    devices
}

/// Read one BAR from a standard PCI endpoint (header type 0).
///
/// This reads the currently assigned address; it does not probe BAR sizes
/// or change the device's configuration.
pub fn read_bar(device: &PciDevice, index: u8) -> Option<PciBar> {
    if device.header_type & 0x7F != 0 || index >= 6 {
        return None;
    }

    let offset = 0x10 + index * 4;

    let low = unsafe { read_config(device.bus, device.device, device.function, offset) };

    if low == 0 {
        return Some(PciBar::Unused);
    }

    if low & 1 != 0 {
        return Some(PciBar::Io {
            port: (low & 0xFFFF_FFFC) as u16,
        });
    }

    let memory_type = (low >> 1) & 0b11;
    let prefetchable = low & 0b1000 != 0;

    match memory_type {
        0 => Some(PciBar::Memory32 {
            address: low & 0xFFFF_FFF0,
            prefetchable,
        }),
        2 if index < 5 => {
            let high =
                unsafe { read_config(device.bus, device.device, device.function, offset + 4) };

            Some(PciBar::Memory64 {
                address: ((high as u64) << 32) | ((low & 0xFFFF_FFF0) as u64),
                prefetchable,
            })
        }
        _ => None,
    }
}

/// Enable PCI memory-space access and bus mastering for a device.
///
/// Memory-space access allows MMIO register access; bus mastering allows
/// the device to initiate DMA transactions to system RAM.
pub fn enable_bus_mastering(device: &PciDevice) -> bool {
    let address = 0x8000_0000u32
        | ((device.bus as u32) << 16)
        | ((device.device as u32) << 11)
        | ((device.function as u32) << 8)
        | 0x04;

    unsafe {
        let mut address_port = Port::<u32>::new(CONFIG_ADDRESS);
        address_port.write(address);

        // Access only the 16-bit PCI command register. Do not write back
        // the adjacent status register, whose bits may be write-one-to-clear.
        let mut command_port = Port::<u16>::new(CONFIG_DATA);
        let command = command_port.read();
        command_port.write(command | (1 << 1) | (1 << 2));

        // Verify both Memory Space Enable and Bus Master Enable.
        address_port.write(address);
        let updated_command = command_port.read();

        updated_command & 0x0006 == 0x0006
    }
}
