// =============================================================================
// Arctos OS — VGA Text Mode Buffer Driver
// =============================================================================
//
// Writes directly to the memory-mapped VGA text buffer at physical address
// 0xB8000. This is the simplest possible display output — an 80×25 grid of
// colored ASCII characters. Each cell is 2 bytes: [ASCII char][color attr].
//
// This module will be replaced by a framebuffer-based GUI in Phase 6.
// For now it gives us essential print/debug capabilities.
// =============================================================================

use core::fmt;
use lazy_static::lazy_static;
use spin::Mutex;
use volatile::Volatile;

// ---------------------------------------------------------------------------
// Color definitions
// ---------------------------------------------------------------------------

/// The 16 standard VGA text-mode colors.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
    Black      = 0x0,
    Blue       = 0x1,
    Green      = 0x2,
    Cyan       = 0x3,
    Red        = 0x4,
    Magenta    = 0x5,
    Brown      = 0x6,
    LightGray  = 0x7,
    DarkGray   = 0x8,
    LightBlue  = 0x9,
    LightGreen = 0xA,
    LightCyan  = 0xB,
    LightRed   = 0xC,
    Pink       = 0xD,
    Yellow     = 0xE,
    White      = 0xF,
}

/// A VGA color attribute byte: 4 bits foreground | 4 bits background.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct ColorCode(u8);

impl ColorCode {
    pub const fn new(foreground: Color, background: Color) -> Self {
        ColorCode((background as u8) << 4 | (foreground as u8))
    }
}

// ---------------------------------------------------------------------------
// Screen character
// ---------------------------------------------------------------------------

/// A single character cell in the VGA buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct ScreenChar {
    ascii_character: u8,
    color_code: ColorCode,
}

// ---------------------------------------------------------------------------
// Buffer geometry
// ---------------------------------------------------------------------------

const BUFFER_HEIGHT: usize = 25;
const BUFFER_WIDTH: usize = 80;

/// The VGA text buffer — a 2D array of volatile screen characters.
#[repr(transparent)]
struct Buffer {
    chars: [[Volatile<ScreenChar>; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

// ---------------------------------------------------------------------------
// Writer
// ---------------------------------------------------------------------------

/// Writes formatted strings to the VGA text buffer.
///
/// Tracks the current column position and color, automatically wraps lines
/// and scrolls the screen when the bottom is reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub x_start: usize,
    pub y_start: usize,
    pub width: usize,
    pub height: usize,
}

pub struct Writer {
    pub column_position: usize,
    pub row_position: usize,
    pub window: Window,
    color_code: ColorCode,
    buffer: &'static mut Buffer,
}

impl Writer {
    /// Write a single ASCII byte to the buffer.
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            0x08 => { // Backspace control character
                if self.column_position > 0 {
                    self.column_position -= 1;
                    let row = BUFFER_HEIGHT - 1;
                    let col = self.column_position;
                    let blank = ScreenChar {
                        ascii_character: b' ',
                        color_code: self.color_code,
                    };
                    self.buffer.chars[row][col].write(blank);
                } else {
                    // Backspace at the start of the line: scroll down to go back to the previous line
                    self.scroll_down();
                    
                    // Find the last typed character in the restored bottom row
                    let row = BUFFER_HEIGHT - 1;
                    let mut last_char_col = BUFFER_WIDTH - 1;
                    while last_char_col > 0 {
                        let character = self.buffer.chars[row][last_char_col].read();
                        if character.ascii_character != b' ' {
                            break;
                        }
                        last_char_col -= 1;
                    }
                    
                    self.column_position = last_char_col + 1;
                    if self.column_position >= BUFFER_WIDTH {
                        self.column_position = BUFFER_WIDTH - 1;
                    }

                    // Delete the character at the new cursor position
                    if self.column_position > 0 {
                        self.column_position -= 1;
                        let col = self.column_position;
                        let blank = ScreenChar {
                            ascii_character: b' ',
                            color_code: self.color_code,
                        };
                        self.buffer.chars[row][col].write(blank);
                    }
                }
            }
            byte => {
                if self.column_position >= BUFFER_WIDTH {
                    self.new_line();
                }

                let row = BUFFER_HEIGHT - 1;
                let col = self.column_position;

                let color_code = self.color_code;
                self.buffer.chars[row][col].write(ScreenChar {
                    ascii_character: byte,
                    color_code,
                });
                self.column_position += 1;
            }
        }
        self.update_hardware_cursor();
    }

    /// Writes a single character cell at an absolute coordinate.
    #[allow(dead_code)]
    pub fn write_char_at(&mut self, row: usize, col: usize, character: u8, color: ColorCode) {
        if row < BUFFER_HEIGHT && col < BUFFER_WIDTH {
            self.buffer.chars[row][col].write(ScreenChar {
                ascii_character: character,
                color_code: color,
            });
        }
    }

    /// Writes a string at an absolute coordinate.
    #[allow(dead_code)]
    pub fn write_string_at(&mut self, row: usize, mut col: usize, s: &str, color: ColorCode) {
        for byte in s.bytes() {
            if col >= BUFFER_WIDTH {
                break;
            }
            self.write_char_at(row, col, byte, color);
            col += 1;
        }
    }

    /// Write a string, converting non-printable/non-ASCII characters to `■`.
    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                // Printable ASCII range, newline, or backspace
                0x20..=0x7e | b'\n' | 0x08 => self.write_byte(byte),
                // Not part of printable ASCII — show a placeholder
                _ => self.write_byte(0xfe), // ■
            }
        }
    }

    /// Scroll the entire screen up by one row.
    fn new_line(&mut self) {
        for row in 1..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                let character = self.buffer.chars[row][col].read();
                self.buffer.chars[row - 1][col].write(character);
            }
        }
        self.clear_row(BUFFER_HEIGHT - 1);
        self.column_position = 0;
    }

    /// Scroll the entire screen down by one row (restoring the previous line to the bottom).
    fn scroll_down(&mut self) {
        for row in (1..BUFFER_HEIGHT).rev() {
            for col in 0..BUFFER_WIDTH {
                let character = self.buffer.chars[row - 1][col].read();
                self.buffer.chars[row][col].write(character);
            }
        }
        self.clear_row(0);
    }

    /// Fill a row with blank spaces.
    fn clear_row(&mut self, row: usize) {
        let blank = ScreenChar {
            ascii_character: b' ',
            color_code: self.color_code,
        };
        for col in 0..BUFFER_WIDTH {
            self.buffer.chars[row][col].write(blank);
        }
    }

    /// Change the current writing color.
    #[allow(dead_code)]
    pub fn set_color(&mut self, foreground: Color, background: Color) {
        self.color_code = ColorCode::new(foreground, background);
    }

    /// Clear the entire screen.
    #[allow(dead_code)]
    pub fn clear_screen(&mut self) {
        for row in 0..BUFFER_HEIGHT {
            self.clear_row(row);
        }
        self.column_position = 0;
        self.update_hardware_cursor();
    }

    /// Updates the hardware blinking cursor to match the current column position.
    pub fn update_hardware_cursor(&self) {
        use x86_64::instructions::port::Port;
        let pos = (BUFFER_HEIGHT - 1) * BUFFER_WIDTH + self.column_position;
        let mut addr_port = Port::new(0x3d4);
        let mut data_port = Port::new(0x3d5);
        unsafe {
            addr_port.write(0x0f_u8);
            data_port.write((pos & 0xff) as u8);
            addr_port.write(0x0e_u8);
            data_port.write(((pos >> 8) & 0xff) as u8);
        }
    }
}

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Global writer instance
// ---------------------------------------------------------------------------

lazy_static! {
    /// The global VGA writer, protected by a spinlock.
    ///
    /// Access via the `print!` and `println!` macros defined below.
    pub static ref WRITER: Mutex<Writer> = Mutex::new(Writer {
        column_position: 0,
        row_position: 0,
        window: Window {
            x_start: 0,
            y_start: 0,
            width: BUFFER_WIDTH,
            height: BUFFER_HEIGHT,
        },
        color_code: ColorCode::new(Color::White, Color::Black),
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
    });
}

// ---------------------------------------------------------------------------
// Print macros
// ---------------------------------------------------------------------------

/// Like `print!` in the standard library, but writes to the VGA text buffer.
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::vga_buffer::_print(format_args!($($arg)*)));
}

/// Like `println!` in the standard library, but writes to the VGA text buffer.
#[macro_export]
macro_rules! println {
    ()            => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

/// Internal print function — called by the macros, not for direct use.
#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    // We print inside an interrupt-free context to prevent deadlocks.
    // If an interrupt occurred while the WRITER lock was held and that
    // interrupt tried to print, it would wait forever for the lock.
    interrupts::without_interrupts(|| {
        WRITER.lock().write_fmt(args).unwrap();
    });
}