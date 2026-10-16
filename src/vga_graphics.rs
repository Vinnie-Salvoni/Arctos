// =============================================================================
// Arctos OS — VGA Graphics Mode 13h Driver
// =============================================================================

use x86_64::instructions::port::Port;
use alloc::vec::Vec;
use spin::Mutex;

// The dimensions of VGA Mode 13h
pub const SCREEN_WIDTH: usize = 320;
pub const SCREEN_HEIGHT: usize = 200;
const FRAMEBUFFER_PHYS_ADDR: u64 = 0xA0000;

// Thread-safe global variables to track graphics state and font mappings
pub static GRAPHICS_CONTEXT: Mutex<Option<GraphicsContext>> = Mutex::new(None);
pub static mut PHYS_MEM_OFFSET: u64 = 0;
static SAVED_FONT: Mutex<[u8; 8192]> = Mutex::new([0u8; 8192]);

pub struct GraphicsContext {
    pub phys_mem_offset: u64,
    pub backbuffer: Vec<u8>,
}

// =============================================================================
// VGA Registers & Magic Mode 13h Values
// =============================================================================

const MISC_WRITE: u8 = 0x63;

const NUM_SEQUENCER_REGS: usize = 5;
const SEQ_REGS: [u8; NUM_SEQUENCER_REGS] = [0x03, 0x01, 0x0F, 0x00, 0x0E];

const NUM_CRTC_REGS: usize = 25;
const CRTC_REGS: [u8; NUM_CRTC_REGS] = [
    0x5F, 0x4F, 0x50, 0x82, 0x54, 0x80, 0xBF, 0x1F,
    0x00, 0x41, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x9C, 0x0E, 0x8F, 0x28, 0x40, 0x96, 0xB9, 0xA3,
    0xFF,
];

const NUM_GRAPHICS_REGS: usize = 9;
const GRAPHICS_REGS: [u8; NUM_GRAPHICS_REGS] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x05, 0x0F,
    0xFF,
];

const NUM_ATTRIBUTE_REGS: usize = 21;
const ATTRIBUTE_REGS: [u8; NUM_ATTRIBUTE_REGS] = [
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
    0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F,
    0x41, 0x00, 0x0F, 0x00, 0x00,
];

// =============================================================================
// Font Mapping Helpers
// =============================================================================

/// Configures the VGA card's memory controller to map Plane 2 directly to 
/// 0xA0000 (mode = true), allowing raw font glyph copying.
fn map_vga_font(mode: bool) {
    unsafe {
        if mode {
            // Enable font access (map Plane 2 for CPU write/read)
            Port::<u8>::new(0x3C4).write(0x00); Port::<u8>::new(0x3C5).write(0x01); // Reset
            Port::<u8>::new(0x3C4).write(0x02); Port::<u8>::new(0x3C5).write(0x04); // Write to plane 2
            Port::<u8>::new(0x3C4).write(0x04); Port::<u8>::new(0x3C5).write(0x04); // Memory Mode
            Port::<u8>::new(0x3C4).write(0x00); Port::<u8>::new(0x3C5).write(0x03); // Clear Reset

            Port::<u8>::new(0x3CE).write(0x04); Port::<u8>::new(0x3CF).write(0x02); // Read from plane 2
            Port::<u8>::new(0x3CE).write(0x05); Port::<u8>::new(0x3CF).write(0x00); // Write mode 0, read mode 0
            Port::<u8>::new(0x3CE).write(0x06); Port::<u8>::new(0x3CF).write(0x00); // Miscellaneous (map to 0xA0000)
        } else {
            // Restore text mode mapping (Plane 0/1 for text/attribute)
            Port::<u8>::new(0x3C4).write(0x00); Port::<u8>::new(0x3C5).write(0x01);
            Port::<u8>::new(0x3C4).write(0x02); Port::<u8>::new(0x3C5).write(0x03); // Write to plane 0 & 1
            Port::<u8>::new(0x3C4).write(0x04); Port::<u8>::new(0x3C5).write(0x02); // Memory Mode
            Port::<u8>::new(0x3C4).write(0x00); Port::<u8>::new(0x3C5).write(0x03);

            Port::<u8>::new(0x3CE).write(0x04); Port::<u8>::new(0x3CF).write(0x00);
            Port::<u8>::new(0x3CE).write(0x05); Port::<u8>::new(0x3CF).write(0x10);
            Port::<u8>::new(0x3CE).write(0x06); Port::<u8>::new(0x3CF).write(0x0E); // Miscellaneous (map to 0xB8000)
        }
    }
}

// =============================================================================
// Initialization and Mode Toggling
// =============================================================================

/// Writes configuration registers to reprogram the VGA hardware.
pub fn init_graphics(phys_mem_offset: u64) {
    unsafe { PHYS_MEM_OFFSET = phys_mem_offset; }

    // 1. Backup the text font from Plane 2 before starting Mode 13h
    map_vga_font(true);
    let font_ptr = (phys_mem_offset + FRAMEBUFFER_PHYS_ADDR) as *const u8;
    unsafe {
        core::ptr::copy_nonoverlapping(font_ptr, SAVED_FONT.lock().as_mut_ptr(), 8192);
    }
    map_vga_font(false);

    // 2. Initialize context with a blank backbuffer
    let context = GraphicsContext {
        phys_mem_offset,
        backbuffer: alloc::vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT],
    };
    *GRAPHICS_CONTEXT.lock() = Some(context);

    // 3. Reprogram the VGA hardware registers
    unsafe {
        Port::<u8>::new(0x3C2).write(MISC_WRITE);

        for idx in 0..NUM_SEQUENCER_REGS {
            Port::<u8>::new(0x3C4).write(idx as u8);
            Port::<u8>::new(0x3C5).write(SEQ_REGS[idx]);
        }

        for idx in 0..NUM_CRTC_REGS {
            Port::<u8>::new(0x3D4).write(idx as u8);
            Port::<u8>::new(0x3D5).write(CRTC_REGS[idx]);
        }

        for idx in 0..NUM_GRAPHICS_REGS {
            Port::<u8>::new(0x3CE).write(idx as u8);
            Port::<u8>::new(0x3CF).write(GRAPHICS_REGS[idx]);
        }

        for idx in 0..NUM_ATTRIBUTE_REGS {
            let mut reset_port = Port::<u8>::new(0x3DA);
            let _ = reset_port.read();
            
            Port::<u8>::new(0x3C0).write(idx as u8);
            Port::<u8>::new(0x3C0).write(ATTRIBUTE_REGS[idx]);
        }

        let mut reset_port = Port::<u8>::new(0x3DA);
        let _ = reset_port.read();
        Port::<u8>::new(0x3C0).write(0x20); // Enable video
    }
}


/// Restores the VGA hardware to the standard 80x25 BIOS text mode.
pub fn restore_text_mode() {

    const TEXT_MISC: u8 = 0x67;
    const TEXT_SEQ: [u8; 5] = [0x03, 0x00, 0x03, 0x00, 0x02];
    const TEXT_CRTC: [u8; 25] = [
        0x5F, 0x4F, 0x50, 0x82, 0x55, 0x81, 0xBF, 0x1F,
        0x00, 0x4F, 0x0D, 0x0E, 0x00, 0x00, 0x00, 0x00,
        0x9C, 0x8E, 0x8F, 0x28, 0x1F, 0x96, 0xB9, 0xA3,
        0xFF,
    ];
    const TEXT_GRAPHICS: [u8; 9] = [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x0E, 0x00,
        0xFF,
    ];
    const TEXT_ATTRIBUTE: [u8; 21] = [
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x14, 0x07,
        0x38, 0x39, 0x3A, 0x3B, 0x3C, 0x3D, 0x3E, 0x3F,
        0x0C, 0x00, 0x0F, 0x08, 0x00,
    ];

    unsafe {
        // Write Miscellaneous Output Register
        Port::<u8>::new(0x3C2).write(TEXT_MISC);

        // Write Sequencer Registers
        // Step 1: Synchronous Reset (bit 1 = 0, bit 0 = 1)
        Port::<u8>::new(0x3C4).write(0x00u8);
        Port::<u8>::new(0x3C5).write(0x01u8);

        for idx in 1..5 {
            Port::<u8>::new(0x3C4).write(idx as u8);
            Port::<u8>::new(0x3C5).write(TEXT_SEQ[idx]);
        }

        // Step 2: Clear Reset (bit 1 = 1, bit 0 = 1)
        Port::<u8>::new(0x3C4).write(0x00u8);
        Port::<u8>::new(0x3C5).write(0x03u8);

        // Unlock CRTC registers (clear bit 7 of register 0x11)
        Port::<u8>::new(0x3D4).write(0x11u8);
        let crtc11 = Port::<u8>::new(0x3D5).read();
        Port::<u8>::new(0x3D5).write(crtc11 & 0x7F);

        // Write CRTC Registers
        for idx in 0..25 {
            Port::<u8>::new(0x3D4).write(idx as u8);
            Port::<u8>::new(0x3D5).write(TEXT_CRTC[idx]);
        }

        // Write Graphics Controller Registers
        for idx in 0..9 {
            Port::<u8>::new(0x3CE).write(idx as u8);
            Port::<u8>::new(0x3CF).write(TEXT_GRAPHICS[idx]);
        }

        // Write Attribute Controller Registers
        for idx in 0..21 {
            let mut reset_port = Port::<u8>::new(0x3DA);
            let _ = reset_port.read();
            
            Port::<u8>::new(0x3C0).write(idx as u8);
            Port::<u8>::new(0x3C0).write(TEXT_ATTRIBUTE[idx]);
        }

        // Re-enable Attribute Controller
        let mut reset_port = Port::<u8>::new(0x3DA);
        let _ = reset_port.read();
        Port::<u8>::new(0x3C0).write(0x20); // Enable video flag
    }

    // Natively reload the embedded font to fix missing text after BGA reset
    load_embedded_font();

    // Free the backbuffer vector allocation (64,000 bytes) from the heap
    *GRAPHICS_CONTEXT.lock() = None;
}

pub fn load_embedded_font() {
    let phys_mem_offset = *crate::memory_mapper::PHYSICAL_MEMORY_OFFSET.lock();
    if phys_mem_offset == 0 {
        return;
    }
    
    map_vga_font(true);
    let font_ptr = (phys_mem_offset + FRAMEBUFFER_PHYS_ADDR) as *mut u8;
    
    unsafe {
        for char_code in 0usize..256 {
            let offset = char_code * 32;
            let glyph = if char_code < 128 {
                crate::vga_font::BASIC_FONTS[char_code]
            } else {
                [0u8; 8]
            };
            
            // In standard 80x25 text mode (16 scanlines per row), placing 8x8 glyphs
            // directly in rows 3..11 gives natural 8x16 proportions (cap height ~8-9 scanlines)
            // with 3 blank lines above for accents/margins and 4 blank lines below for spacing.
            for row in 0..16 {
                let byte = if (3..11).contains(&row) {
                    let src_row = row - 3;
                    glyph[src_row].reverse_bits()
                } else {
                    0u8
                };
                core::ptr::write_volatile(font_ptr.add(offset + row), byte);
            }
            for row in 16..32 {
                core::ptr::write_volatile(font_ptr.add(offset + row), 0u8);
            }
        }
    }
    
    map_vga_font(false);
}

// =============================================================================
// Drawing Functions (Writes to the Backbuffer)
// =============================================================================

pub fn write_pixel(x: usize, y: usize, color: u8) {
    if let Some(ref mut ctx) = *GRAPHICS_CONTEXT.lock() {
        if x < SCREEN_WIDTH && y < SCREEN_HEIGHT {
            let offset = y * SCREEN_WIDTH + x;
            ctx.backbuffer[offset] = color;
        }
    }
}

pub fn clear_screen(color: u8) {
    if let Some(ref mut ctx) = *GRAPHICS_CONTEXT.lock() {
        for pixel in ctx.backbuffer.iter_mut() {
            *pixel = color;
        }
    }
}

pub fn draw_char(x: usize, y: usize, c: char, color: u8) {
    let font_guard = SAVED_FONT.lock();
    let char_code = c as usize;
    if char_code < 256 {
        let offset = char_code * 32;
        for row in 0..16 {
            let row_byte = font_guard[offset + row];
            for col in 0..8 {
                let bit = (row_byte >> (7 - col)) & 1;
                if bit != 0 {
                    write_pixel(x + col, y + row, color);
                }
            }
        }
    }
}

pub fn draw_string(x: usize, y: usize, text: &str, color: u8) {
    let mut cur_x = x;
    for c in text.chars() {
        draw_char(cur_x, y, c, color);
        cur_x += 8;
    }
}

pub fn draw_rect(x: usize, y: usize, w: usize, h: usize, color: u8) {
    if w == 0 || h == 0 { return; }
    for i in 0..w {
        write_pixel(x + i, y, color);
        write_pixel(x + i, y + h - 1, color);
    }
    for j in 0..h {
        write_pixel(x, y + j, color);
        write_pixel(x + w - 1, y + j, color);
    }
}

pub fn draw_rect_filled(x: usize, y: usize, w: usize, h: usize, color: u8) {
    for j in 0..h {
        for i in 0..w {
            write_pixel(x + i, y + j, color);
        }
    }
}

pub fn draw_line(x1: usize, y1: usize, x2: usize, y2: usize, color: u8) {
    let mut x1_val = x1 as isize;
    let mut y1_val = y1 as isize;
    let x2_val = x2 as isize;
    let y2_val = y2 as isize;

    let dx = (x2_val - x1_val).abs();
    let dy = (y2_val - y1_val).abs();
    let sx = if x1_val < x2_val { 1 } else { -1 };
    let sy = if y1_val < y2_val { 1 } else { -1 };
    let mut err = dx - dy;

    loop {
        if x1_val >= 0 && x1_val < SCREEN_WIDTH as isize && y1_val >= 0 && y1_val < SCREEN_HEIGHT as isize {
            write_pixel(x1_val as usize, y1_val as usize, color);
        }
        if x1_val == x2_val && y1_val == y2_val { break; }
        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x1_val += sx;
        }
        if e2 < dx {
            err += dx;
            y1_val += sy;
        }
    }
}

// =============================================================================
// Presenting/Buffer Swapping
// =============================================================================

/// Copies the backbuffer to the physical memory at 0xA0000.
pub fn swap_buffers() {
    if let Some(ref ctx) = *GRAPHICS_CONTEXT.lock() {
        let dest_ptr = (ctx.phys_mem_offset + FRAMEBUFFER_PHYS_ADDR) as *mut u8;
        
        unsafe {
            core::ptr::copy_nonoverlapping(
                ctx.backbuffer.as_ptr(),
                dest_ptr,
                SCREEN_WIDTH * SCREEN_HEIGHT,
            );
        }
    }
}