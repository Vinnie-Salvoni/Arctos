// =============================================================================
// Arctos OS — Text-Based Kernel Shell
// =============================================================================

use crate::allocator::heap_stats;
use crate::fs;
use crate::interrupts::SHELL_QUEUE;
use crate::vga_buffer::{Color, ColorCode, Window, WRITER};

use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use pc_keyboard::DecodedKey;
use spin::Mutex;

static ALLOCATED_CHUNKS: Mutex<Vec<Box<[u8]>>> = Mutex::new(Vec::new());
static FORMAT_CONFIRMATION: Mutex<bool> = Mutex::new(false);

const COMMANDS: &[&str] = &[
    "help", "nethelp", "info", "clear", "echo", "print", "alloc", "free", "ls", "dir", "cat",
    "format", "confirm", "cancel", "reboot", "restart", "shutdown", "poweroff", "pci", "netinfo",
    "nettest", "netrx", "netloop",
];

// ---------------------------------------------------------------------------
// Main Shell Loop
// ---------------------------------------------------------------------------

pub fn run() -> ! {
    {
        let mut writer = WRITER.lock();
        writer.window = Window {
            x_start: 0,
            y_start: 0,
            width: 80,
            height: 25,
        };
        writer.column_position = 0;
        writer.row_position = 0;
        writer.set_color(Color::White, Color::Black);
        writer.clear_screen();
    }

    crate::println!("Arctos OS — Kernel Shell");
    crate::println!("Text mode: VGA 80x25");
    crate::println!("Type 'help' to list commands.");
    print_prompt();

    let mut input_buffer = String::new();

    loop {
        let key = {
            let mut queue = SHELL_QUEUE.lock();
            queue.pop_front()
        };

        if let Some(key) = key {
            match key {
                DecodedKey::RawKey(_) => {}

                DecodedKey::Unicode(character) => match character {
                    '\n' => {
                        clear_ghost_suggestions();
                        crate::println!("");

                        let command = input_buffer.trim().to_string();
                        if !command.is_empty() {
                            execute_command(&command);
                        }

                        input_buffer.clear();
                        print_prompt();
                    }

                    '\u{0008}' => {
                        if !input_buffer.is_empty() {
                            input_buffer.pop();

                            {
                                let mut writer = WRITER.lock();
                                writer.write_byte(0x08);
                            }

                            clear_ghost_suggestions();
                            draw_ghost_suggestion(&input_buffer);
                        }
                    }

                    '\t' => {
                        if let Some(suggestion) = get_autocomplete_match(&input_buffer) {
                            clear_prompt_line(input_buffer.len());
                            input_buffer = String::from(suggestion);
                            crate::print!("{}", input_buffer);
                            clear_ghost_suggestions();
                        }
                    }

                    character => {
                        if character.is_ascii()
                            && !character.is_control()
                            && input_buffer.len() < 60
                        {
                            input_buffer.push(character);

                            {
                                let mut writer = WRITER.lock();
                                writer.write_byte(character as u8);
                            }

                            clear_ghost_suggestions();
                            draw_ghost_suggestion(&input_buffer);
                        }
                    }
                },
            }
        }

        x86_64::instructions::hlt();
    }
}

// ---------------------------------------------------------------------------
// Help
// ---------------------------------------------------------------------------

fn print_help() {
    crate::println!("--- Arctos Shell ---");
    crate::println!("help              Show general commands");
    crate::println!("nethelp           Show networking commands");
    crate::println!("info              Kernel and heap information");
    crate::println!("clear             Clear the terminal");
    crate::println!("echo <text>       Print text");
    crate::println!("alloc [bytes]     Allocate a test memory block");
    crate::println!("free              Release the last test allocation");
    crate::println!("ls / dir          List stored files");
    crate::println!("cat <filename>    Read a stored file");
    crate::println!("format            Request filesystem formatting");
    crate::println!("confirm           Confirm pending formatting");
    crate::println!("cancel            Cancel pending formatting");
    crate::println!("reboot / restart  Restart the machine");
    crate::println!("shutdown          Request machine shutdown");
    crate::println!("netloop           Test E1000 transmission and reception in a loop");
}

fn print_nethelp() {
    crate::println!("--- Arctos Networking Commands ---");
    crate::println!("");
    crate::println!("Device discovery");
    crate::println!("  pci             Scan PCI devices");
    crate::println!("  netinfo         Inspect network controllers");
    crate::println!("");
    crate::println!("Network tests");
    crate::println!("  nettest         Test E1000 transmission");
    crate::println!("  netrx           Poll for received Ethernet frames");
    crate::println!("  netloop         Test local Ethernet loopback");
    crate::println!("");
    crate::println!("Note: nettest verifies TX descriptor completion.");
    crate::println!("It does not prove that a remote device received the frame.");
    crate::println!("netrx requires incoming traffic to test reception.");
}

// ---------------------------------------------------------------------------
// Shell Commands
// ---------------------------------------------------------------------------

fn execute_command(cmd: &str) {
    let (command_name, args) = cmd.split_once(char::is_whitespace).unwrap_or((cmd, ""));
    let command_string = command_name.to_ascii_lowercase();
    let command = command_string.as_str();
    let args = args.trim();

    match command {
        "help" => {
            if !args.is_empty() {
                crate::println!("[error] Usage: help");
                return;
            }
            print_help();
        }

        "nethelp" => {
            if !args.is_empty() {
                crate::println!("[error] Usage: nethelp");
                return;
            }
            print_nethelp();
        }

        "info" => {
            if !args.is_empty() {
                crate::println!("[error] Usage: info");
                return;
            }

            let stats = heap_stats();
            crate::println!("--- Arctos OS ---");
            crate::println!("Version: {}", env!("CARGO_PKG_VERSION"));
            crate::println!("Architecture: x86_64");
            crate::println!("Display: VGA text mode (80x25)");
            crate::println!("Kernel heap used: {} bytes", stats.0);
            crate::println!("Kernel heap free: {} bytes", stats.1);
        }

        "clear" => {
            if !args.is_empty() {
                crate::println!("[error] Usage: clear");
                return;
            }
            let mut writer = WRITER.lock();
            writer.clear_screen();
        }

        "echo" | "print" => {
            crate::println!("{}", args);
        }

        "alloc" => {
            const MAX_ALLOCATION: usize = 8 * 1024;
            const MAX_ACTIVE_ALLOCATIONS: usize = 8;

            let size = if args.is_empty() {
                1024
            } else {
                match args.parse::<usize>() {
                    Ok(size) if size > 0 => size,
                    _ => {
                        crate::println!("[error] Usage: alloc [positive byte count]");
                        return;
                    }
                }
            };

            if size > MAX_ALLOCATION {
                crate::println!(
                    "[error] Maximum test allocation is {} bytes.",
                    MAX_ALLOCATION
                );
                return;
            }

            if ALLOCATED_CHUNKS.lock().len() >= MAX_ACTIVE_ALLOCATIONS {
                crate::println!("[error] Too many active test allocations. Run 'free' first.");
                return;
            }

            let chunk = alloc::vec![0x5Au8; size].into_boxed_slice();

            if chunk.iter().all(|byte| *byte == 0x5A) {
                crate::println!(
                    "[ok] Allocated and verified {} bytes at {:p}.",
                    size,
                    chunk.as_ptr()
                );
                ALLOCATED_CHUNKS.lock().push(chunk);
            } else {
                crate::println!("[error] Allocation verification failed.");
            }
        }

        "free" => {
            if !args.is_empty() {
                crate::println!("[error] Usage: free");
                return;
            }

            let chunk = ALLOCATED_CHUNKS.lock().pop();

            if let Some(chunk) = chunk {
                let size = chunk.len();
                let verified = chunk.iter().all(|byte| *byte == 0x5A);
                drop(chunk);

                if verified {
                    crate::println!("[ok] Released {} bytes.", size);
                } else {
                    crate::println!("[warning] Memory contents changed; block released.");
                }
            } else {
                crate::println!("[info] No test allocations to release.");
            }
        }

        "ls" | "dir" => {
            if !args.is_empty() {
                crate::println!("[error] Usage: ls");
                return;
            }

            if !fs::mount() {
                crate::println!("[error] Could not mount the filesystem. Nothing was formatted.");
                return;
            }

            let files = fs::list_files();

            if files.is_empty() {
                crate::println!("No files found.");
            } else {
                crate::println!("--- Files ---");
                for name in files {
                    crate::println!("{}", name);
                }
            }
        }

        "cat" => {
            if args.is_empty() {
                crate::println!("Usage: cat <filename>");
                return;
            }

            if !fs::mount() {
                crate::println!("[error] Could not mount the filesystem. Nothing was formatted.");
                return;
            }

            match fs::read_file(args) {
                Some(data) => match core::str::from_utf8(&data) {
                    Ok(contents) => crate::println!("{}", contents),
                    Err(_) => {
                        crate::println!("[info] File contains binary data ({} bytes).", data.len());
                    }
                },
                None => crate::println!("[error] File '{}' not found.", args),
            }
        }

        "pci" => {
            if !args.is_empty() {
                crate::println!("[error] Usage: pci");
                return;
            }

            crate::println!("[PCI] Scanning PCI devices...");
            let devices = crate::pci::scan();

            if devices.is_empty() {
                crate::println!("[PCI] No devices found.");
                return;
            }

            for device in devices.iter() {
                crate::println!(
                    "Bus {:02X} Device {:02X} Function {} | Vendor {:04X} Device {:04X} | Class {:02X}:{:02X}",
                    device.bus,
                    device.device,
                    device.function,
                    device.vendor_id,
                    device.device_id,
                    device.class_code,
                    device.subclass
                );

                if device.class_code == 0x02 {
                    crate::println!("  [network] Network controller detected.");

                    for index in 0..6 {
                        match crate::pci::read_bar(device, index) {
                            Some(crate::pci::PciBar::Unused) => {}
                            Some(crate::pci::PciBar::Io { port }) => {
                                crate::println!("  BAR{}: I/O port 0x{:04X}", index, port);
                            }
                            Some(crate::pci::PciBar::Memory32 {
                                address,
                                prefetchable,
                            }) => {
                                crate::println!(
                                    "  BAR{}: 32-bit memory 0x{:08X}, prefetchable={}",
                                    index,
                                    address,
                                    prefetchable
                                );
                            }
                            Some(crate::pci::PciBar::Memory64 {
                                address,
                                prefetchable,
                            }) => {
                                crate::println!(
                                    "  BAR{}: 64-bit memory 0x{:016X}, prefetchable={}",
                                    index,
                                    address,
                                    prefetchable
                                );
                            }
                            None => crate::println!("  BAR{}: unsupported or invalid", index),
                        }
                    }
                }
            }

            crate::println!("[PCI] Scan complete.");
        }

        "netinfo" => {
            if !args.is_empty() {
                crate::println!("[error] Usage: netinfo");
                return;
            }

            crate::println!("[NET] Scanning for network controllers...");
            let devices = crate::pci::scan();
            let mut found = 0usize;

            for device in devices.iter() {
                if device.class_code != 0x02 {
                    continue;
                }

                found += 1;
                crate::println!("");
                crate::println!(
                    "[NET] Controller {}: PCI {:02X}:{:02X}.{}",
                    found,
                    device.bus,
                    device.device,
                    device.function
                );
                crate::println!(
                    "  Vendor: 0x{:04X} | Device: 0x{:04X}",
                    device.vendor_id,
                    device.device_id
                );

                let supported = device.vendor_id == 0x8086 && device.device_id == 0x100E;
                crate::println!(
                    "  Driver: {}",
                    if supported {
                        "Intel E1000 (82540EM)"
                    } else {
                        "Unidentified / unsupported"
                    }
                );

                if !supported {
                    crate::println!("  Status: Detected; compatible driver not implemented.");
                } else {
                    let bar0 = match crate::pci::read_bar(device, 0) {
                        Some(crate::pci::PciBar::Memory32 { address, .. }) => Some(address as u64),
                        Some(crate::pci::PciBar::Memory64 { address, .. }) => Some(address),
                        _ => None,
                    };

                    match bar0 {
                        Some(address) => match unsafe { crate::net::e1000::read_mac(address) } {
                            Some(mac) => {
                                crate::println!(
                                    "  MAC address: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                                    mac[0],
                                    mac[1],
                                    mac[2],
                                    mac[3],
                                    mac[4],
                                    mac[5]
                                );
                                crate::println!("  Status: MAC address read successfully.");
                            }
                            None => crate::println!("  ERROR: Could not read a valid MAC address."),
                        },
                        None => crate::println!("  ERROR: No usable MMIO BAR0 found."),
                    }
                }

                crate::println!("  PCI BARs:");
                for index in 0..6 {
                    match crate::pci::read_bar(device, index) {
                        Some(crate::pci::PciBar::Unused) => {}
                        Some(crate::pci::PciBar::Io { port }) => {
                            crate::println!("    BAR{}: I/O port 0x{:04X}", index, port);
                        }
                        Some(crate::pci::PciBar::Memory32 {
                            address,
                            prefetchable,
                        }) => {
                            crate::println!(
                                "    BAR{}: MMIO 0x{:08X}, prefetchable={}",
                                index,
                                address,
                                prefetchable
                            );
                        }
                        Some(crate::pci::PciBar::Memory64 {
                            address,
                            prefetchable,
                        }) => {
                            crate::println!(
                                "    BAR{}: MMIO 0x{:016X}, prefetchable={}",
                                index,
                                address,
                                prefetchable
                            );
                        }
                        None => crate::println!("    BAR{}: unsupported or invalid", index),
                    }
                }
            }

            if found == 0 {
                crate::println!("[NET] No PCI network controllers found.");
            } else {
                crate::println!("");
                crate::println!("[NET] Scan complete: {} controller(s) detected.", found);
            }
        }

        "nettest" => {
            if !args.is_empty() {
                crate::println!("[error] Usage: nettest");
                return;
            }

            crate::println!("[NET] Searching for Intel E1000...");
            let devices = crate::pci::scan();

            let device = match devices
                .iter()
                .find(|d| d.vendor_id == 0x8086 && d.device_id == 0x100E)
            {
                Some(device) => *device,
                None => {
                    crate::println!("[error] Intel E1000 (8086:100E) not found.");
                    return;
                }
            };

            let bar0 = match crate::pci::read_bar(&device, 0) {
                Some(crate::pci::PciBar::Memory32 { address, .. }) => address as u64,
                Some(crate::pci::PciBar::Memory64 { address, .. }) => address,
                _ => {
                    crate::println!("[error] E1000 BAR0 is not a memory BAR.");
                    return;
                }
            };

            crate::println!("[NET] E1000 found at MMIO {:#X}", bar0);
            crate::println!("[NET] Initialising DMA transmit ring...");
            let mut driver = crate::net::e1000::E1000Driver::new(device, bar0);

            if let Err(error) = crate::net::driver::NetworkDriver::init(&mut driver) {
                crate::println!("[error] E1000 initialisation failed: {:?}", error);
                return;
            }

            let info = crate::net::driver::NetworkDriver::info(&driver);
            crate::println!(
                "[NET] MAC: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                info.mac_address[0],
                info.mac_address[1],
                info.mac_address[2],
                info.mac_address[3],
                info.mac_address[4],
                info.mac_address[5]
            );
            crate::println!(
                "[NET] Link reported: {}",
                if info.link_up { "UP" } else { "DOWN" }
            );

            let mut frame = [0u8; 60];
            frame[..6].fill(0xFF);
            frame[6..12].copy_from_slice(&info.mac_address);
            frame[12..14].copy_from_slice(&0x88B5u16.to_be_bytes());

            let payload = b"ARCTOS E1000 DMA TRANSMIT TEST";
            frame[14..14 + payload.len()].copy_from_slice(payload);

            crate::println!("[NET] Transmitting {}-byte test frame...", frame.len());

            match crate::net::driver::NetworkDriver::transmit(&mut driver, &frame) {
                Ok(()) => {
                    crate::println!("[NET] TX SUCCESS: descriptor completed.");
                    crate::println!(
                        "[NET] This confirms hardware processed the TX descriptor; \
                         it does not yet confirm remote reception."
                    );
                }
                Err(error) => crate::println!("[error] TX failed: {:?}", error),
            }
        }

        "netrx" => {
            if !args.is_empty() {
                crate::println!("[error] Usage: netrx");
                return;
            }

            crate::println!("[NET] Searching for Intel E1000...");
            let devices = crate::pci::scan();

            let device = match devices
                .iter()
                .find(|d| d.vendor_id == 0x8086 && d.device_id == 0x100E)
            {
                Some(device) => *device,
                None => {
                    crate::println!("[error] Intel E1000 not found.");
                    return;
                }
            };

            let bar0 = match crate::pci::read_bar(&device, 0) {
                Some(crate::pci::PciBar::Memory32 { address, .. }) => address as u64,
                Some(crate::pci::PciBar::Memory64 { address, .. }) => address,
                _ => {
                    crate::println!("[error] Invalid E1000 BAR0.");
                    return;
                }
            };

            let mut driver = crate::net::e1000::E1000Driver::new(device, bar0);
            if let Err(error) = crate::net::driver::NetworkDriver::init(&mut driver) {
                crate::println!("[error] E1000 initialisation failed: {:?}", error);
                return;
            }

            crate::println!("[NET] RX ring initialised.");
            crate::println!("[NET] Polling for incoming Ethernet frames...");
            let mut frame = [0u8; 2048];

            for _ in 0..1_000_000usize {
                match crate::net::driver::NetworkDriver::receive(&mut driver, &mut frame) {
                    Ok(Some(length)) => {
                        if length >= 14 {
                            crate::println!("[NET] Received {}-byte frame", length);
                            crate::println!(
                                "[NET] Destination: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                                frame[0],
                                frame[1],
                                frame[2],
                                frame[3],
                                frame[4],
                                frame[5]
                            );
                            crate::println!(
                                "[NET] Source: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                                frame[6],
                                frame[7],
                                frame[8],
                                frame[9],
                                frame[10],
                                frame[11]
                            );
                            crate::println!(
                                "[NET] EtherType: 0x{:02X}{:02X}",
                                frame[12],
                                frame[13]
                            );
                        } else {
                            crate::println!("[NET] Received a short Ethernet frame.");
                        }
                        return;
                    }
                    Ok(None) => core::hint::spin_loop(),
                    Err(error) => {
                        crate::println!("[error] RX failed: {:?}", error);
                        return;
                    }
                }
            }

            crate::println!("[NET] No frame received during this polling window.");
            crate::println!(
                "[NET] RX is polling-only; incoming traffic must be generated to test it."
            );
        }
        "netloop" => {
            if !args.is_empty() {
                crate::println!("[error] Usage: netloop");
                return;
            }

            crate::println!("[NET] Searching for Intel E1000...");
            let devices = crate::pci::scan();

            let device = match devices
                .iter()
                .find(|d| d.vendor_id == 0x8086 && d.device_id == 0x100E)
            {
                Some(device) => *device,
                None => {
                    crate::println!("[error] Intel E1000 (8086:100E) not found.");
                    return;
                }
            };

            let bar0 = match crate::pci::read_bar(&device, 0) {
                Some(crate::pci::PciBar::Memory32 { address, .. }) => address as u64,
                Some(crate::pci::PciBar::Memory64 { address, .. }) => address,
                _ => {
                    crate::println!("[error] Invalid E1000 BAR0.");
                    return;
                }
            };

            let mut driver = crate::net::e1000::E1000Driver::new(device, bar0);

            if let Err(error) = crate::net::driver::NetworkDriver::init(&mut driver) {
                crate::println!("[error] E1000 initialisation failed: {:?}", error);
                return;
            }

            if let Err(error) = driver.enable_loopback() {
                crate::println!("[error] Could not enable loopback: {:?}", error);
                return;
            }

            match driver.phy_debug_status() {
                Ok(bmcr) => {
                    crate::println!("[NET] PHY BMCR: {:#06X}", bmcr);
                    crate::println!(
                        "[NET] PHY loopback bit: {}",
                        if bmcr & (1 << 14) != 0 {
                            "SET"
                        } else {
                            "CLEAR"
                        }
                    );
                }
                Err(error) => {
                    crate::println!("[NET] PHY status read failed: {:?}", error);
                }
            }
            let info = crate::net::driver::NetworkDriver::info(&driver);

            let mut sent = [0u8; 60];

            // Broadcast destination: FF:FF:FF:FF:FF:FF
            sent[..6].fill(0xFF);

            // Keep the adapter's MAC address as the source.
            sent[6..12].copy_from_slice(&info.mac_address);

            sent[12..14].copy_from_slice(&0x88B5u16.to_be_bytes());

            let payload = b"ARCTOS E1000 LOOPBACK TEST";
            sent[14..14 + payload.len()].copy_from_slice(payload);

            crate::println!("[NET] MAC loopback enabled.");
            crate::println!("[NET] Transmitting test frame...");

            if let Err(error) = crate::net::driver::NetworkDriver::transmit(&mut driver, &sent) {
                let _ = driver.disable_loopback();
                crate::println!("[error] TX failed: {:?}", error);
                return;
            }
            match driver.tx_debug_snapshot() {
                Ok((tdh, tdt, tdlen, tctl, status)) => {
                    crate::println!(
                        "[NET] TX after send: TDH={} TDT={} TDLEN={} TCTL={:#010X}",
                        tdh,
                        tdt,
                        tdlen,
                        tctl
                    );
                    crate::println!("[NET] Last TX descriptor status: {:#04X}", status);
                }
                Err(error) => {
                    crate::println!("[NET] TX diagnostic failed: {:?}", error);
                }
            }
            let mut received = [0u8; 2048];

            for _ in 0..5_000_000usize {
                match crate::net::driver::NetworkDriver::receive(&mut driver, &mut received) {
                    Ok(Some(length)) => {
                        let _ = driver.disable_loopback();

                        if length == sent.len() && received[..length] == sent[..] {
                            crate::println!("[NET] LOOPBACK SUCCESS.");
                            crate::println!(
                                "[NET] Received frame exactly matches transmitted frame ({} bytes).",
                                length
                            );
                        } else {
                            crate::println!(
                                "[NET] Received a frame, but it did not match the test frame ({} bytes).",
                                length
                            );
                        }

                        return;
                    }
                    Ok(None) => core::hint::spin_loop(),
                    Err(error) => {
                        let _ = driver.disable_loopback();
                        crate::println!("[error] RX failed: {:?}", error);
                        return;
                    }
                }
            }

            match driver.rx_debug_snapshot() {
                Ok((statuses, rctl, rdh, rdt, rdlen, rdbal, rdbah, first_rx_buffer)) => {
                    crate::println!("[NET] No loopback frame received.");
                    crate::println!("[NET] RCTL: {:#010X}", rctl);
                    crate::println!("[NET] RDH: {}  RDT: {}", rdh, rdt);
                    crate::println!("[NET] RDLEN: {}", rdlen);
                    crate::println!("[NET] RDBAL: {:#010X}", rdbal);
                    crate::println!("[NET] RDBAH: {:#010X}", rdbah);
                    crate::println!("[NET] RXD[0] buffer physical: {:#018X}", first_rx_buffer);
                    let mut completed = 0usize;

                    for (index, status) in statuses.iter().enumerate() {
                        if status & 1 != 0 {
                            completed += 1;
                            crate::println!("[NET] RXD[{}]: status={:#04X}", index, status);
                        }
                    }

                    crate::println!(
                        "[NET] Descriptors marked complete: {} / {}",
                        completed,
                        statuses.len()
                    );
                }
                Err(error) => {
                    crate::println!("[NET] RX diagnostic failed: {:?}", error);
                }
            }

            let _ = driver.disable_loopback();
        }
        "format" => {
            if !args.is_empty() {
                crate::println!("[error] Usage: format");
                return;
            }

            if fs::mount() {
                crate::println!("[error] A valid AIVFS filesystem already exists.");
                crate::println!("[info] Refusing to overwrite it.");
                return;
            }

            *FORMAT_CONFIRMATION.lock() = true;
            crate::println!("[warning] Formatting overwrites filesystem metadata.");
            crate::println!("[warning] Current target: primary IDE slave.");
            crate::println!("[warning] Only proceed with the disposable QEMU test disk.");
            crate::println!("Type 'confirm' to proceed, or 'cancel' to abort.");
        }

        "confirm" => {
            if !args.is_empty() {
                crate::println!("[error] Usage: confirm");
                return;
            }

            let pending = {
                let mut confirmation = FORMAT_CONFIRMATION.lock();
                let pending = *confirmation;
                *confirmation = false;
                pending
            };

            if !pending {
                crate::println!("[info] No pending format operation.");
                return;
            }

            if fs::mount() {
                crate::println!("[error] A valid AIVFS filesystem already exists.");
                crate::println!("[info] Refusing to overwrite it.");
                return;
            }

            if fs::format() && fs::mount() {
                crate::println!("[ok] Filesystem formatted and mounted.");
            } else {
                crate::println!("[error] Formatting or verification failed.");
            }
        }

        "cancel" => {
            if !args.is_empty() {
                crate::println!("[error] Usage: cancel");
                return;
            }
            *FORMAT_CONFIRMATION.lock() = false;
            crate::println!("[info] Pending format operation cancelled.");
        }

        "reboot" | "restart" => {
            if !args.is_empty() {
                crate::println!("[error] Usage: reboot");
                return;
            }

            crate::println!("[system] Rebooting...");
            let mut port = x86_64::instructions::port::Port::<u8>::new(0x64);

            unsafe {
                port.write(0xFE);
            }

            loop {
                x86_64::instructions::hlt();
            }
        }

        "shutdown" | "poweroff" => {
            if !args.is_empty() {
                crate::println!("[error] Usage: shutdown");
                return;
            }

            crate::println!("[system] Requesting shutdown...");
            let mut acpi_port = x86_64::instructions::port::Port::<u16>::new(0x604);

            unsafe {
                acpi_port.write(0x2000);
            }

            let mut bochs_port = x86_64::instructions::port::Port::<u8>::new(0x501);

            unsafe {
                bochs_port.write(0x34);
            }

            loop {
                x86_64::instructions::hlt();
            }
        }

        "" => {}

        _ => {
            crate::println!("[error] Unknown command: '{}'", command);
            crate::println!("Type 'help' to see available commands.");
        }
    }
}

// ---------------------------------------------------------------------------
// Prompt and Autocomplete Helpers
// ---------------------------------------------------------------------------

pub fn print_prompt() {
    let mut writer = WRITER.lock();
    writer.set_color(Color::LightCyan, Color::Black);
    writer.write_string("arctos > ");
    writer.set_color(Color::White, Color::Black);
}

fn clear_prompt_line(input_len: usize) {
    let mut writer = WRITER.lock();
    let start_col = 9;
    let end_col = (start_col + input_len).min(80);
    let blank = ColorCode::new(Color::White, Color::Black);
    let row = 24;

    for col in start_col..end_col {
        writer.write_char_at(row, col, b' ', blank);
    }

    writer.column_position = start_col;
    writer.update_hardware_cursor();
}

fn get_autocomplete_match(input: &str) -> Option<&'static str> {
    if input.is_empty() {
        return None;
    }

    let lower_input = input.to_ascii_lowercase();

    COMMANDS
        .iter()
        .copied()
        .find(|command| command.starts_with(&lower_input))
}

fn draw_ghost_suggestion(input: &str) {
    if let Some(suggestion) = get_autocomplete_match(input) {
        let suffix = &suggestion[input.len()..];
        let mut writer = WRITER.lock();
        let start_col = writer.column_position.min(79);
        let ghost_color = ColorCode::new(Color::DarkGray, Color::Black);
        let row = 24;

        for (offset, byte) in suffix.bytes().enumerate() {
            let col = start_col + offset;
            if col >= 80 {
                break;
            }
            writer.write_char_at(row, col, byte, ghost_color);
        }
    }
}

fn clear_ghost_suggestions() {
    let mut writer = WRITER.lock();
    let start_col = writer.column_position.min(80);
    let blank_color = ColorCode::new(Color::White, Color::Black);
    let row = 24;

    for col in start_col..80 {
        writer.write_char_at(row, col, b' ', blank_color);
    }
}
