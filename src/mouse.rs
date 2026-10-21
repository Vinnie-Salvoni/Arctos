// =============================================================================
// Arctos OS — PS/2 Mouse Driver
// =============================================================================
//
// Hardware Architecture:
//   - Port 0x60: PS/2 Data Port
//   - Port 0x64: PS/2 Controller Command / Status Register
//   - IRQ 12: Secondary PS/2 Mouse Interrupt (Vector 44)
// =============================================================================

use spin::Mutex;
use x86_64::instructions::port::Port;

// Screen bounds for clamping mouse movement (Default 1080p: 1920x1080)
pub const SCREEN_WIDTH: isize = 1920;
pub const SCREEN_HEIGHT: isize = 1080;

/// Represents the current physical state of the mouse device
#[derive(Debug, Clone, Copy)]
pub struct MouseState {
    pub x: isize,
    pub y: isize,
    pub left_button: bool,
    pub right_button: bool,
    pub middle_button: bool,
}

pub static MOUSE_STATE: Mutex<MouseState> = Mutex::new(MouseState {
    x: 960, // Center screen x
    y: 540, // Center screen y
    left_button: false,
    right_button: false,
    middle_button: false,
});

// PS/2 Mouse Packet Parsing State Machine
static PACKET_CYCLE: Mutex<u8> = Mutex::new(0);
static PACKET_BYTES: Mutex<[u8; 3]> = Mutex::new([0u8; 3]);

// Helper I/O functions
fn wait_write() {
    let mut status_port = Port::<u8>::new(0x64);
    let mut timeout = 100_000;
    while timeout > 0 {
        if (unsafe { status_port.read() } & 2) == 0 {
            return;
        }
        timeout -= 1;
    }
}

fn wait_read() {
    let mut status_port = Port::<u8>::new(0x64);
    let mut timeout = 100_000;
    while timeout > 0 {
        if (unsafe { status_port.read() } & 1) != 0 {
            return;
        }
        timeout -= 1;
    }
}

fn mouse_write(data: u8) {
    wait_write();
    let mut command_port = Port::<u8>::new(0x64);
    unsafe {
        command_port.write(0xD4);
    } // Signal PS/2 controller to route byte to mouse
    wait_write();
    let mut data_port = Port::<u8>::new(0x60);
    unsafe {
        data_port.write(data);
    }
}

fn mouse_read() -> u8 {
    wait_read();
    let mut data_port = Port::<u8>::new(0x60);
    unsafe { data_port.read() }
}

/// Initializes the hardware PS/2 controller and enables the auxiliary mouse device.
pub fn init_mouse() {
    // 1. Enable Auxiliary Device (Secondary PS/2 Port)
    wait_write();
    let mut command_port = Port::<u8>::new(0x64);
    unsafe {
        command_port.write(0xA8);
    }

    // 2. Read Controller Configuration Byte (CCB)
    wait_write();
    unsafe {
        command_port.write(0x20);
    }
    let mut ccb = mouse_read();

    // Enable IRQ 12 (Bit 1) and clear Disable Mouse Clock (Bit 5)
    ccb |= 1 << 1;
    ccb &= !(1 << 5);

    // Write updated CCB back to PS/2 Controller
    wait_write();
    unsafe {
        command_port.write(0x60);
    }
    wait_write();
    let mut data_port = Port::<u8>::new(0x60);
    unsafe {
        data_port.write(ccb);
    }

    // 3. Send "Use Default Settings" command (0xF6) to mouse
    mouse_write(0xF6);
    let _ack1 = mouse_read(); // Expect 0xFA ACK

    // 4. Send "Enable Data Reporting" command (0xF4) to mouse
    mouse_write(0xF4);
    let _ack2 = mouse_read(); // Expect 0xFA ACK
}

/// Processes a single byte received from PS/2 IRQ 12 interrupt handler.
pub fn handle_mouse_byte(byte: u8) {
    let mut cycle = PACKET_CYCLE.lock();
    let mut bytes = PACKET_BYTES.lock();

    match *cycle {
        0 => {
            // Byte 0 must have Bit 3 set (Always 1 in standard PS/2 protocol).
            // If bit 3 is not 1, we are out of sync with the 3-byte sequence!
            if (byte & 0x08) != 0 {
                bytes[0] = byte;
                *cycle = 1;
            }
        }
        1 => {
            bytes[1] = byte;
            *cycle = 2;
        }
        2 => {
            bytes[2] = byte;
            *cycle = 0; // Reset state machine for next 3-byte packet

            // Parse complete packet
            let flags = bytes[0];
            let raw_x = bytes[1];
            let raw_y = bytes[2];

            let left = (flags & 0x01) != 0;
            let right = (flags & 0x02) != 0;
            let middle = (flags & 0x04) != 0;

            // Extract signed relative movements
            let rel_x = if (flags & 0x10) != 0 {
                raw_x as isize - 256
            } else {
                raw_x as isize
            };

            let rel_y = if (flags & 0x20) != 0 {
                raw_y as isize - 256
            } else {
                raw_y as isize
            };

            // Ignore extreme overflow spikes
            if (flags & 0xC0) == 0 {
                let mut state = MOUSE_STATE.lock();
                state.x = (state.x + rel_x).clamp(0, SCREEN_WIDTH - 1);
                // PS/2 Y axis is inverted relative to screen coordinates
                state.y = (state.y - rel_y).clamp(0, SCREEN_HEIGHT - 1);
                state.left_button = left;
                state.right_button = right;
                state.middle_button = middle;
            }
        }
        _ => *cycle = 0,
    }
}
