// =============================================================================
// Arctos OS — UART 16550 Serial Driver (COM1 @ Port 0x3F8)
// Enables host-assisted AI bridge streaming over QEMU serial socket.
// All reads have a tick-based timeout so a missing bridge can never freeze the kernel.
// =============================================================================

use alloc::string::String;
use core::sync::atomic::Ordering;
use spin::Mutex;
use x86_64::instructions::port::Port;

const COM1_BASE: u16 = 0x3F8;

pub static SERIAL1: Mutex<SerialPort> = Mutex::new(SerialPort::new(COM1_BASE));

/// Track whether the bridge successfully replied at least once.
static BRIDGE_ALIVE: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

pub struct SerialPort {
    data: Port<u8>,
    int_en: Port<u8>,
    fifo_ctrl: Port<u8>,
    line_ctrl: Port<u8>,
    modem_ctrl: Port<u8>,
    line_status: Port<u8>,
}

impl SerialPort {
    pub const fn new(base: u16) -> SerialPort {
        SerialPort {
            data: Port::new(base),
            int_en: Port::new(base + 1),
            fifo_ctrl: Port::new(base + 2),
            line_ctrl: Port::new(base + 3),
            modem_ctrl: Port::new(base + 4),
            line_status: Port::new(base + 5),
        }
    }

    pub fn init(&mut self) {
        unsafe {
            self.int_en.write(0x00); // Disable all interrupts
            self.line_ctrl.write(0x80); // Enable DLAB (baud rate divisor)
            self.data.write(0x03); // Divisor = 3 → 38400 baud, low byte
            self.int_en.write(0x00); // high byte
            self.line_ctrl.write(0x03); // 8 bits, no parity, 1 stop bit
            self.fifo_ctrl.write(0xC7); // Enable FIFO, clear, 14-byte threshold
            self.modem_ctrl.write(0x0B); // IRQs enabled, RTS/DSR set
        }
    }

    pub fn send_byte(&mut self, byte: u8) {
        unsafe {
            // Transmit-Hold-Register-Empty bit (bit 5). Timeout after 10000 polls.
            let mut tries = 0u32;
            while (self.line_status.read() & 0x20) == 0 {
                tries += 1;
                if tries > 10_000 {
                    return; // Bridge not draining — give up rather than hang
                }
            }
            self.data.write(byte);
        }
    }

    pub fn send_string(&mut self, s: &str) {
        for b in s.bytes() {
            self.send_byte(b);
        }
    }

    pub fn receive_byte(&mut self) -> Option<u8> {
        unsafe {
            if (self.line_status.read() & 0x01) != 0 {
                Some(self.data.read())
            } else {
                None
            }
        }
    }

    pub fn byte_available(&mut self) -> bool {
        unsafe { (self.line_status.read() & 0x01) != 0 }
    }

    /// Read bytes until \n or 30-second timeout expires.
    pub fn read_line_with_timeout(&mut self) -> Option<String> {
        let mut buf = String::new();
        let start_ticks = crate::interrupts::TIMER_TICKS.load(Ordering::Relaxed);
        let mut polls = 0u64;

        loop {
            polls += 1;
            let current_ticks = crate::interrupts::TIMER_TICKS.load(Ordering::Relaxed);

            // Timeout after 30 seconds of timer ticks (30,000 ms) OR 300,000 calibrated loop iterations
            if (current_ticks > start_ticks && current_ticks - start_ticks >= 30_000)
                || polls > 300_000
            {
                if !buf.is_empty() {
                    return Some(buf);
                }
                return None;
            }

            if let Some(b) = self.receive_byte() {
                if b == b'\n' || b == b'\r' {
                    if !buf.is_empty() {
                        return Some(buf);
                    }
                } else {
                    buf.push(b as char);
                }
            }

            // Calibrated 0.1ms spin delay per loop iteration (300,000 iterations = 30 seconds)
            for _ in 0..100_000 {
                core::hint::spin_loop();
            }
        }
    }
}

pub fn init() {
    SERIAL1.lock().init();
}

/// Send a prompt to the host AI bridge.
/// Returns false immediately if the bridge appears offline.
pub fn send_prompt(prompt: &str) {
    let mut serial = SERIAL1.lock();
    serial.send_string(prompt);
    serial.send_byte(b'\n');
}

pub fn receive_byte_now() -> Option<u8> {
    let mut serial = SERIAL1.lock();
    if serial.byte_available() {
        serial.receive_byte()
    } else {
        None
    }
}

/// Receive one response line from the host AI bridge.
/// Returns a descriptive error string (never blocks forever).
pub fn receive_response() -> String {
    match SERIAL1.lock().read_line_with_timeout() {
        Some(resp) => {
            BRIDGE_ALIVE.store(true, Ordering::Relaxed);
            resp
        }
        None => {
            let was_alive = BRIDGE_ALIVE.load(Ordering::Relaxed);
            if was_alive {
                String::from(
                    "[ArcAI] Bridge timed out. Is 'python scripts/ai_bridge.py' still running?",
                )
            } else {
                String::from("[ArcAI] Bridge not connected. Run: python scripts/ai_bridge.py")
            }
        }
    }
}
