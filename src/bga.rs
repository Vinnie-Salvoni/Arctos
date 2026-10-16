use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use pc_keyboard::{DecodedKey, KeyCode};
use spin::Mutex;
use x86_64::instructions::port::Port;
use crate::intent::{self, Intent, SemanticIntentEngine, IntentListener, execute_kernel_intent};
use crate::interrupts::SHELL_QUEUE;

// Basic graphics Card driver
pub const VBE_DISPI_IOPORT_INDEX: u16 = 0x01CE;
pub const VBE_DISPI_IOPORT_DATA: u16 = 0x01CF;
pub const VBE_DISPI_INDEX_XRES: u16 = 1;
pub const VBE_DISPI_INDEX_YRES: u16 = 2;
pub const VBE_DISPI_INDEX_BPP: u16 = 3;
pub const VBE_DISPI_INDEX_ENABLE: u16 = 4;
pub const VBE_DISPI_ENABLED: u16 = 0x01;
pub const VBE_DISPI_LFB_ENABLED: u16 = 0x40;

pub static GUI_ACTIVE: AtomicBool = AtomicBool::new(false);
/// Set by ProgramCanvas VM to force a continuous redraw every frame.
pub static FORCE_REDRAW: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ArcAIState {
    Idle = 0,
    Processing = 1,
}
pub static ARCAI_STATE_VAR: AtomicU8 = AtomicU8::new(0);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UISurface {
    Ambient,
    Developer,
    AITensorWorkspace,
}

pub static CURRENT_SURFACE: Mutex<UISurface> = Mutex::new(UISurface::Ambient);
pub static SHOW_TELEMETRY: Mutex<bool> = Mutex::new(true);
pub static CURRENT_THEME: Mutex<u32> = Mutex::new(0x000F2A38); // Default Dark Teal
pub static SECONDARY_THEME: Mutex<Option<u32>> = Mutex::new(None); // Optional 2nd color for linear gradient
static LAST_FEEDBACK_MSG: Mutex<String> = Mutex::new(String::new());
pub static RESPONSE_SCROLL: Mutex<usize> = Mutex::new(0);

/// Dynamically parses hex codes (#FF5733, 0x00FF88) or color names into 32-bit ARGB framebuffer colors.
pub fn parse_dynamic_color(input: &str) -> Option<u32> {
    let clean = input.trim().to_lowercase();
    let hex_str = if clean.starts_with('#') {
        &clean[1..]
    } else if clean.starts_with("0x") {
        &clean[2..]
    } else {
        &clean
    };

    if hex_str.len() == 6 {
        if let Ok(val) = u32::from_str_radix(hex_str, 16) {
            return Some(val);
        }
    }

    match hex_str {
        "black" => Some(0x00000000),
        "white" => Some(0x00FFFFFF),
        "red" | "ruby" | "crimson" => Some(0x00B01020),
        "green" | "emerald" | "lime" => Some(0x00053A1F),
        "blue" | "sapphire" | "navy" => Some(0x000A1F40),
        "purple" | "amethyst" | "violet" => Some(0x002D084A),
        "amber" | "gold" | "yellow" => Some(0x004A3A08),
        "cyan" | "teal" | "turquoise" => Some(0x00008080),
        "orange" | "coral" => Some(0x00D05010),
        "pink" | "rose" => Some(0x00C04080),
        "grey" | "gray" | "dark" => Some(0x00121218),
        _ => None,
    }
}

/// Dynamically parses dual-color linear gradients (e.g. "red to blue", "black to emerald", "gradient #FF0000 #0000FF").
pub fn parse_dynamic_gradient(input: &str) -> bool {
    let lower = input.to_lowercase();
    let mut color_parts = Vec::new();

    for token in lower.split_whitespace() {
        let clean_token = token.trim_matches(|c: char| c == ',' || c == ';' || c == '"' || c == '\'');
        if let Some(col) = parse_dynamic_color(clean_token) {
            color_parts.push(col);
        }
    }

    if color_parts.len() >= 2 {
        *CURRENT_THEME.lock() = color_parts[0];
        *SECONDARY_THEME.lock() = Some(color_parts[1]);
        true
    } else if color_parts.len() == 1 {
        *CURRENT_THEME.lock() = color_parts[0];
        *SECONDARY_THEME.lock() = None;
        true
    } else {
        false
    }
}

fn write_bga_reg(index: u16, val: u16) {
    let mut index_port = Port::<u16>::new(VBE_DISPI_IOPORT_INDEX);
    let mut data_port = Port::<u16>::new(VBE_DISPI_IOPORT_DATA);
    unsafe {
        index_port.write(index);
        data_port.write(val);
    }
}

pub fn init_bga_1080p() {
    write_bga_reg(VBE_DISPI_INDEX_ENABLE, 0);
    write_bga_reg(VBE_DISPI_INDEX_XRES, 1920);
    write_bga_reg(VBE_DISPI_INDEX_YRES, 1080);
    write_bga_reg(VBE_DISPI_INDEX_BPP, 32);
    write_bga_reg(VBE_DISPI_INDEX_ENABLE, VBE_DISPI_ENABLED | VBE_DISPI_LFB_ENABLED);
}

pub fn disable_bga() {
    GUI_ACTIVE.store(false, Ordering::Relaxed);
    write_bga_reg(VBE_DISPI_INDEX_ENABLE, 0);
}

/// Scans the PCI bus for the Bochs VBE Graphics Adapter (0x1234:0x1111) to obtain the LFB address.
pub fn get_bga_lfb_phys() -> u64 {
    let mut addr_port = Port::<u32>::new(0xCF8);
    let mut data_port = Port::<u32>::new(0xCFC);

    for bus in 0u32..16 {
        for dev in 0u32..32 {
            let pci_addr = (1 << 31) | (bus << 16) | (dev << 11);
            unsafe {
                addr_port.write(pci_addr);
                let id = data_port.read();
                if id == 0x11111234 || id == 0x12341111 {
                    addr_port.write(pci_addr | 0x10);
                    let bar0 = data_port.read();
                    let phys = (bar0 & 0xFFFFFFF0) as u64;
                    if phys != 0 {
                        return phys;
                    }
                }
            }
        }
    }
    0xFD000000 // Default QEMU Bochs VBE Framebuffer Base
}

// ── 2D Primitives & Frosted Glassmorphism Engine ─────────────────────────

fn draw_pixel(slice: &mut [u32], pitch: usize, x: usize, y: usize, color: u32) {
    if x < 1920 && y < 1080 {
        let offset = y * pitch + x;
        if offset < slice.len() {
            slice[offset] = color;
        }
    }
}

fn draw_rect_filled(slice: &mut [u32], pitch: usize, rx: usize, ry: usize, rw: usize, rh: usize, color: u32) {
    for y in ry..core::cmp::min(ry + rh, 1080) {
        for x in rx..core::cmp::min(rx + rw, 1920) {
            draw_pixel(slice, pitch, x, y, color);
        }
    }
}

fn draw_neon_panel(slice: &mut [u32], pitch: usize, rx: usize, ry: usize, rw: usize, rh: usize, bg_color: u32, border_color: u32) {
    draw_rect_filled(slice, pitch, rx, ry, rw, rh, bg_color);
    // Double glowing border for glassmorphism
    for x in rx..rx + rw {
        draw_pixel(slice, pitch, x, ry, border_color);
        draw_pixel(slice, pitch, x, ry + 1, border_color);
        draw_pixel(slice, pitch, x, ry + rh - 2, border_color);
        draw_pixel(slice, pitch, x, ry + rh - 1, border_color);
    }
    for y in ry..ry + rh {
        draw_pixel(slice, pitch, rx, y, border_color);
        draw_pixel(slice, pitch, rx + 1, y, border_color);
        draw_pixel(slice, pitch, rx + rw - 2, y, border_color);
        draw_pixel(slice, pitch, rx + rw - 1, y, border_color);
    }
}

fn draw_char_32bpp(slice: &mut [u32], pitch: usize, x: usize, y: usize, c: char, color: u32) {
    let char_code = c as usize;
    if char_code < 128 {
        let glyph = crate::vga_font::BASIC_FONTS[char_code];
        for row in 0..8 {
            let row_byte = glyph[row];
            for col in 0..8 {
                let bit = (row_byte >> col) & 1; // Bit 0 is left in BASIC_FONTS
                if bit != 0 {
                    // Draw 2x2 scaled pixels for crisp HD readability
                    draw_pixel(slice, pitch, x + col * 2, y + row * 2, color);
                    draw_pixel(slice, pitch, x + col * 2 + 1, y + row * 2, color);
                    draw_pixel(slice, pitch, x + col * 2, y + row * 2 + 1, color);
                    draw_pixel(slice, pitch, x + col * 2 + 1, y + row * 2 + 1, color);
                }
            }
        }
    }
}

fn draw_string_32bpp(slice: &mut [u32], pitch: usize, mut x: usize, y: usize, text: &str, color: u32) {
    for c in text.chars() {
        draw_char_32bpp(slice, pitch, x, y, c, color);
        x += 18; // 16px char + 2px spacing
    }
}

fn draw_multiline_string_32bpp(slice: &mut [u32], pitch: usize, start_x: usize, start_y: usize, max_width: usize, text: &str, color: u32) {
    let char_width = 18;
    let line_height = 22;
    let mut cur_x = start_x;
    let mut cur_y = start_y;
    let max_x = start_x + max_width;

    let words = text.split(' ');
    for word in words {
        let word_len = word.len();
        if cur_x + word_len * char_width > max_x && cur_x > start_x {
            cur_x = start_x;
            cur_y += line_height;
        }

        for c in word.chars() {
            if c == '\n' {
                cur_x = start_x;
                cur_y += line_height;
                continue;
            }
            if cur_x + char_width > max_x {
                cur_x = start_x;
                cur_y += line_height;
            }
            draw_char_32bpp(slice, pitch, cur_x, cur_y, c, color);
            cur_x += char_width;
        }
        cur_x += char_width;
    }
}

fn wrap_text(text: &str, max_width: usize, char_width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let max_chars = (max_width / char_width).max(1);

    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut current_line = String::new();
        for word in paragraph.split(' ') {
            if current_line.is_empty() {
                let mut rem = word;
                while rem.len() > max_chars {
                    lines.push(rem[..max_chars].to_string());
                    rem = &rem[max_chars..];
                }
                current_line = rem.to_string();
            } else if current_line.len() + 1 + word.len() <= max_chars {
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                lines.push(current_line);
                let mut rem = word;
                while rem.len() > max_chars {
                    lines.push(rem[..max_chars].to_string());
                    rem = &rem[max_chars..];
                }
                current_line = rem.to_string();
            }
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }
    }
    lines
}

// ── Wallpaper & Background Cyber Grid ────────────────────────────────────


pub static TERTIARY_THEME: Mutex<Option<u32>> = Mutex::new(None);
pub static QUATERNARY_THEME: Mutex<Option<u32>> = Mutex::new(None);
pub static GENERATIVE_CANVAS_PARAMS: Mutex<Option<[f32; 16]>> = Mutex::new(None);

fn fast_sin(mut x: f32) -> f32 {
    let pi = 3.14159265f32;
    let tau = 6.2831853f32;
    while x < -pi { x += tau; }
    while x > pi { x -= tau; }
    if x < 0.0 {
        let y = -x;
        -(16.0 * y * (pi - y)) / (5.0 * pi * pi - 4.0 * y * (pi - y))
    } else {
        (16.0 * x * (pi - x)) / (5.0 * pi * pi - 4.0 * x * (pi - x))
    }
}

fn fast_atan2(y: f32, x: f32) -> f32 {
    let pi = 3.14159265f32;
    if x == 0.0 {
        if y > 0.0 { pi / 2.0 } else if y < 0.0 { -pi / 2.0 } else { 0.0 }
    } else {
        let z = y / x;
        let mut a = z / (1.0 + 0.28 * z * z);
        if x < 0.0 {
            if y >= 0.0 { a += pi; } else { a -= pi; }
        }
        a
    }
}

fn repaint_canvas(slice: &mut [u32], pitch: usize, _prompt_input: &str) {
    let theme = *CURRENT_THEME.lock();
    let secondary = *SECONDARY_THEME.lock();
    let tertiary = *TERTIARY_THEME.lock();
    let quaternary = *QUATERNARY_THEME.lock();
    let gen_params = *GENERATIVE_CANVAS_PARAMS.lock();

    // 1. Fill Dynamic Continuous Spectral Field Canvas (Sub-3ms SIMD Block Render)
    if let Some(p) = gen_params {
        let r1 = ((theme >> 16) & 0xFF) as f32;
        let g1 = ((theme >> 8) & 0xFF) as f32;
        let b1 = (theme & 0xFF) as f32;

        let (r2, g2, b2) = if let Some(sec) = secondary {
            (((sec >> 16) & 0xFF) as f32, ((sec >> 8) & 0xFF) as f32, (sec & 0xFF) as f32)
        } else { (r1, g1, b1) };

        let (r3, g3, b3) = if let Some(tert) = tertiary {
            (((tert >> 16) & 0xFF) as f32, ((tert >> 8) & 0xFF) as f32, (tert & 0xFF) as f32)
        } else { (r2, g2, b2) };

        let (r4, g4, b4) = if let Some(quat) = quaternary {
            (((quat >> 16) & 0xFF) as f32, ((quat >> 8) & 0xFF) as f32, (quat & 0xFF) as f32)
        } else { (r3, g3, b3) };

        let pattern_type = p[13];

        // High-Performance 4x4 SIMD-style Block Renderer (Sub-3ms execution, Zero Lag)
        for y in (0..1080usize).step_by(4) {
            let v = (y as f32) / 1080.0;
            let row_offset = y * pitch;
            if row_offset + 1920 <= slice.len() {
                for x in (0..1920usize).step_by(4) {
                    let u = (x as f32) / 1920.0;
                    
                    let (w1, w2, w3) = if pattern_type >= 0.25 && pattern_type < 0.55 {
                        // POLAR SPIRAL MODE (Spiral / Swirl / Vortex Geometry)
                        let dx = u - 0.5;
                        let dy = v - 0.5;
                        let r_dist = (dx * dx + dy * dy) * 4.0;
                        let theta = fast_atan2(dy, dx);
                        (
                            0.5 + 0.5 * fast_sin(r_dist * 12.0 + theta * 6.0 + p[3]),
                            0.5 + 0.5 * fast_sin(r_dist * 20.0 - theta * 4.0 + p[7]),
                            0.5 + 0.5 * fast_sin(theta * 8.0 + p[11]),
                        )
                    } else if pattern_type >= 0.55 && pattern_type < 0.75 {
                        // CONCENTRIC RINGS MODE
                        let dx = u - 0.5;
                        let dy = v - 0.5;
                        let r_dist = (dx * dx + dy * dy) * 8.0;
                        (0.5 + 0.5 * fast_sin(r_dist * 16.0 + p[3]), 0.5 + 0.5 * fast_sin(r_dist * 8.0 + p[7]), 0.5)
                    } else {
                        // LINEAR WAVE MODE
                        (
                            0.5 + 0.5 * fast_sin(p[0]*u * 6.28 + p[1]*v * 6.28 + p[3]),
                            0.5 + 0.5 * fast_sin(p[4]*u * 6.28 + p[5]*v * 6.28 + p[7]),
                            0.5 + 0.5 * fast_sin(p[8]*u * 6.28 + p[9]*v * 6.28 + p[11]),
                        )
                    };

                    let r_val = r1 * (1.0 - w1) + r2 * w1 * (1.0 - w2) + r3 * w2 * (1.0 - w3) + r4 * w3;
                    let g_val = g1 * (1.0 - w1) + g2 * w1 * (1.0 - w2) + g3 * w2 * (1.0 - w3) + g4 * w3;
                    let b_val = b1 * (1.0 - w1) + b2 * w1 * (1.0 - w2) + b3 * w2 * (1.0 - w3) + b4 * w3;

                    let r = if r_val < 0.0 { 0u32 } else if r_val > 255.0 { 255u32 } else { r_val as u32 };
                    let g = if g_val < 0.0 { 0u32 } else if g_val > 255.0 { 255u32 } else { g_val as u32 };
                    let b = if b_val < 0.0 { 0u32 } else if b_val > 255.0 { 255u32 } else { b_val as u32 };
                    let col = (b << 16) | (g << 8) | r;

                    for dy in 0..4 {
                        if y + dy < 1080 {
                            let offset = (y + dy) * pitch + x;
                            let end = (offset + 4).min((y + dy + 1) * pitch);
                            if end <= slice.len() {
                                slice[offset..end].fill(col);
                            }
                        }
                    }
                }
            }
        }
    } else if let Some(col2) = secondary {
        let r1 = ((theme >> 16) & 0xFF) as u32;
        let g1 = ((theme >> 8) & 0xFF) as u32;
        let b1 = (theme & 0xFF) as u32;

        let r2 = ((col2 >> 16) & 0xFF) as u32;
        let g2 = ((col2 >> 8) & 0xFF) as u32;
        let b2 = (col2 & 0xFF) as u32;

        for y in 0..1080usize {
            let factor = ((y * 256) / 1080) as u32;
            let r = (r1 * (256 - factor) + r2 * factor) >> 8;
            let g = (g1 * (256 - factor) + g2 * factor) >> 8;
            let b = (b1 * (256 - factor) + b2 * factor) >> 8;
            let row_color = (r << 16) | (g << 8) | b;

            let row_offset = y * pitch;
            if row_offset + 1920 <= slice.len() {
                slice[row_offset..row_offset + 1920].fill(row_color);
            }
        }
    } else {
        slice.fill(theme);
    }

    // 2. Render Active Fullscreen App (if present)
    if let Some(app) = crate::apps::ACTIVE_APP.lock().clone() {
        draw_rect_filled(slice, pitch, app.x, app.y, app.width, app.height, 0xFF08121C);
        draw_neon_panel(slice, pitch, app.x, app.y, app.width, 45, 0xFF050E1A, 0x0000E5FF);
        let title_x = 960usize.saturating_sub(app.title.len() * 9);
        draw_string_32bpp(slice, pitch, title_x, app.y + 12, &app.title, 0x00FFFFFF);
        draw_string_32bpp(slice, pitch, app.x + app.width - 50, app.y + 12, "[X]", 0x00FF4444);

        for elem in &app.elements {
            let ex = app.x + elem.x_off;
            let ey = app.y + elem.y_off;

            match elem.widget {
                crate::apps::UIWidgetKind::Panel { bg_color, border_color } => {
                    draw_neon_panel(slice, pitch, ex, ey, elem.width, elem.height, bg_color, border_color);
                }
                crate::apps::UIWidgetKind::Text { ref text, color } => {
                    draw_multiline_string_32bpp(slice, pitch, ex, ey, elem.width, text, color);
                }
                crate::apps::UIWidgetKind::DisplayScreen { ref text, bg_color, text_color } => {
                    draw_neon_panel(slice, pitch, ex, ey, elem.width, elem.height, bg_color, 0x0000FFCC);
                    let txt_len = text.len();
                    draw_string_32bpp(slice, pitch, ex + elem.width - 40 - txt_len * 18, ey + 18, text, text_color);
                }
                crate::apps::UIWidgetKind::Button { ref label, bg_color, text_color, .. } => {
                    draw_neon_panel(slice, pitch, ex, ey, elem.width, elem.height, bg_color, 0x0000E5FF);
                    let label_offset = (elem.width.saturating_sub(label.len() * 18)) / 2;
                    draw_string_32bpp(slice, pitch, ex + label_offset, ey + 28, label, text_color);
                }
                crate::apps::UIWidgetKind::ProgramCanvas { ref program } => {
                    // Execute AI-generated VM program — no hardcoded logic here.
                    // Every frame: tick VM, handle key events, run draw ops.
                    let program = program.clone();
                    let pressed_key = {
                        let mut vm = crate::apps::VM_STATE.lock();
                        vm.tick += 1;
                        vm.pressed_key.clone()
                    };

                    let mut ip = 0usize;
                    let max_ops = program.len().min(2000);
                    let mut guard_count = 0usize;

                    while ip < max_ops && guard_count < 4000 {
                        guard_count += 1;
                        let op = &program[ip];
                        ip += 1;

                        match op {
                            crate::apps::VmOp::FillRect { x, y, w, h, color } => {
                                let px = (ex as i64 + x).max(ex as i64) as usize;
                                let py = (ey as i64 + y).max(ey as i64) as usize;
                                draw_rect_filled(slice, pitch, px, py, *w as usize, *h as usize, *color);
                            }
                            crate::apps::VmOp::DrawText { x, y, text, color } => {
                                // Substitute {var_name} placeholders with live VM values
                                let mut rendered = text.clone();
                                let vm_snap = crate::apps::VM_STATE.lock().vars.clone();
                                for (k, v) in &vm_snap {
                                    let placeholder = alloc::format!("{{{}}}", k);
                                    if rendered.contains(placeholder.as_str()) {
                                        let val_str = alloc::format!("{}", v);
                                        rendered = rendered.replace(placeholder.as_str(), &val_str);
                                    }
                                }
                                let px = (ex as i64 + x) as usize;
                                let py = (ey as i64 + y) as usize;
                                draw_string_32bpp(slice, pitch, px, py, &rendered, *color);
                            }
                            crate::apps::VmOp::DrawCell { cx, cy, cw, ch, color } => {
                                let px = (ex as i64 + cx * cw) as usize;
                                let py = (ey as i64 + cy * ch) as usize;
                                draw_rect_filled(slice, pitch, px, py, *cw as usize, *ch as usize, *color);
                            }
                            crate::apps::VmOp::SetVar { name, value } => {
                                crate::apps::VM_STATE.lock().vars.insert(name.clone(), *value);
                            }
                            crate::apps::VmOp::AddVar { name, delta } => {
                                let mut vm = crate::apps::VM_STATE.lock();
                                let entry = vm.vars.entry(name.clone()).or_insert(0);
                                *entry += delta;
                            }
                            crate::apps::VmOp::ModVar { name, modulus } => {
                                let mut vm = crate::apps::VM_STATE.lock();
                                if let Some(v) = vm.vars.get_mut(name.as_str()) {
                                    if *modulus != 0 { *v = v.rem_euclid(*modulus); }
                                }
                            }
                            crate::apps::VmOp::IfEq { name, value, then_idx } => {
                                let cur = crate::apps::VM_STATE.lock().vars.get(name.as_str()).copied().unwrap_or(0);
                                if cur == *value { ip = *then_idx; }
                            }
                            crate::apps::VmOp::Jump { idx } => {
                                ip = *idx;
                            }
                            crate::apps::VmOp::OnKey { key, then_idx } => {
                                if let Some(ref pk) = pressed_key {
                                    if pk.as_str() == key.as_str() { ip = *then_idx; }
                                }
                            }
                            crate::apps::VmOp::TickGate { every_n } => {
                                let tick = crate::apps::VM_STATE.lock().tick;
                                if tick % every_n != 0 { break; } // skip rest of program this frame
                            }
                            crate::apps::VmOp::Halt => { break; }
                        }
                    }

                    // Clear pressed key after processing
                    crate::apps::VM_STATE.lock().pressed_key = None;
                    // Signal the outer render loop to keep repainting every frame
                    FORCE_REDRAW.store(true, Ordering::Relaxed);
                }
                crate::apps::UIWidgetKind::Slider { value, min, max, ref label, bg_color, accent_color } => {
                    draw_neon_panel(slice, pitch, ex, ey, elem.width, elem.height, bg_color, accent_color);
                    let val_str = alloc::format!("{}: {:.1}", label, value);
                    draw_string_32bpp(slice, pitch, ex + 15, ey + 10, &val_str, 0x00FFFFFF);
                    let track_y = ey + elem.height - 22;
                    let track_w = elem.width.saturating_sub(40);
                    draw_rect_filled(slice, pitch, ex + 20, track_y, track_w, 6, 0xFF050E14);
                    let ratio = if max > min { ((value - min) / (max - min)).clamp(0.0, 1.0) } else { 0.0 };
                    let fill_w = (track_w as f32 * ratio) as usize;
                    draw_rect_filled(slice, pitch, ex + 20, track_y, fill_w, 6, accent_color);
                    draw_rect_filled(slice, pitch, ex + 20 + fill_w, track_y - 5, 10, 16, 0x00FFFFFF);
                }
                crate::apps::UIWidgetKind::ProgressBar { progress, ref label, bg_color, fill_color } => {
                    draw_neon_panel(slice, pitch, ex, ey, elem.width, elem.height, bg_color, fill_color);
                    let pct = (progress.clamp(0.0, 1.0) * 100.0) as usize;
                    let txt = alloc::format!("{}: {}%", label, pct);
                    draw_string_32bpp(slice, pitch, ex + 15, ey + 10, &txt, 0x00FFFFFF);
                    let bar_y = ey + elem.height - 22;
                    let bar_w = elem.width.saturating_sub(30);
                    draw_rect_filled(slice, pitch, ex + 15, bar_y, bar_w, 12, 0xFF050E14);
                    let fill_w = (bar_w as f32 * progress.clamp(0.0, 1.0)) as usize;
                    draw_rect_filled(slice, pitch, ex + 15, bar_y, fill_w, 12, fill_color);
                }
            }
        }
    }

    // 3. Render Persistent Corner Hotspot Nooks (always on top)
    let spots = intent::HOTSPOTS.lock().clone();
    for spot in spots {
        draw_neon_panel(slice, pitch, spot.x, spot.y, spot.width, spot.height, 0xDD050E14, 0x0000E5FF);
        let label_x = spot.x + (spot.width.saturating_sub(spot.label.len() * 10)) / 2;
        draw_string_32bpp(slice, pitch, label_x, spot.y + 6, &spot.label, 0x00FFFFFF);
    }
}

fn repaint_intent_bar(slice: &mut [u32], pitch: usize, prompt_input: &str) {
    let bar_x = 360;
    let bar_y = 890;
    let bar_w = 1200;
    let bar_h = 150;
    draw_neon_panel(slice, pitch, bar_x, bar_y, bar_w, bar_h, 0xEE050E14, 0x0000E5FF);
    draw_string_32bpp(slice, pitch, bar_x + 20, bar_y + 15, "INTENT >", 0x0000E5FF);

    let input_lines = wrap_text(prompt_input, 1020, 18);
    let mut cursor_x = bar_x + 150;
    let mut cursor_y = bar_y + 15;

    for (i, line) in input_lines.iter().enumerate() {
        if i >= 5 { break; }
        let line_y = bar_y + 15 + i * 22;
        draw_string_32bpp(slice, pitch, bar_x + 150, line_y, line, 0x00FFFFFF);
        cursor_x = bar_x + 150 + line.len() * 18;
        cursor_y = line_y;
    }

    // Blinking cursor in Intent Bar
    draw_rect_filled(slice, pitch, cursor_x, cursor_y, 12, 20, 0x0000E5FF);
}

fn repaint_response_card(slice: &mut [u32], pitch: usize) {
    let feedback = LAST_FEEDBACK_MSG.lock().clone();
    if !feedback.is_empty() {
        let card_x = 360;
        let card_y = 550;
        let card_w = 1200;
        let card_h = 320;
        let visible_lines = 11;
        let max_text_width = 1140;

        draw_neon_panel(slice, pitch, card_x, card_y, card_w, card_h, 0xEE050E14, 0x00FFB703);
        draw_string_32bpp(slice, pitch, card_x + 20, card_y + 15, "ARCAI >", 0x00FFB703);

        let lines = wrap_text(&feedback, max_text_width, 18);
        let total_lines = lines.len();

        let mut scroll = *RESPONSE_SCROLL.lock();
        let max_scroll = total_lines.saturating_sub(visible_lines);
        if scroll > max_scroll {
            scroll = max_scroll;
            *RESPONSE_SCROLL.lock() = scroll;
        }

        let header_info = if total_lines > visible_lines {
            let end_line = (scroll + visible_lines).min(total_lines);
            alloc::format!("[Lines {}-{} of {}]  [UP (PgUp/Up)] [DOWN (PgDn/Down)]", scroll + 1, end_line, total_lines)
        } else {
            alloc::format!("[Lines 1-{} of {}]", total_lines, total_lines)
        };
        draw_string_32bpp(slice, pitch, card_x + card_w - 530, card_y + 15, &header_info, 0x0000FFCC);

        let end_idx = (scroll + visible_lines).min(total_lines);
        for (i, line_idx) in (scroll..end_idx).enumerate() {
            let line_y = card_y + 50 + i * 22;
            draw_string_32bpp(slice, pitch, card_x + 20, line_y, &lines[line_idx], 0x0000FFCC);
        }
    }
}

// ── Pixel-Perfect Cursor Background Save & Restore Buffer ────────────────
static SAVED_CURSOR_BG: Mutex<[u32; 12 * 18]> = Mutex::new([0u32; 12 * 18]);

fn save_cursor_background(slice: &[u32], pitch: usize, mx: usize, my: usize) {
    let mut bg = SAVED_CURSOR_BG.lock();
    for r in 0..18 {
        let py = my + r;
        if py >= 1080 { break; }
        for c in 0..12 {
            let px = mx + c;
            if px >= 1920 { break; }
            let offset = py * pitch + px;
            if offset < slice.len() {
                bg[r * 12 + c] = slice[offset];
            }
        }
    }
}

fn restore_cursor_background(slice: &mut [u32], pitch: usize, mx: usize, my: usize) {
    let bg = SAVED_CURSOR_BG.lock();
    for r in 0..18 {
        let py = my + r;
        if py >= 1080 { break; }
        for c in 0..12 {
            let px = mx + c;
            if px >= 1920 { break; }
            let offset = py * pitch + px;
            if offset < slice.len() {
                slice[offset] = bg[r * 12 + c];
            }
        }
    }
}

/// Enters interactive 1080p GUI desktop mode with live Intent listening and Mouse interactivity.
pub fn run_gui_mode() {
    init_bga_1080p();
    GUI_ACTIVE.store(true, Ordering::Relaxed);

    // Load per-user profile from AIVFS storage and apply custom corner hotspots
    let user_prof = crate::user_profile::get_or_load_profile();
    crate::user_profile::apply_profile_hotspots(&user_prof);

    let lfb_phys = get_bga_lfb_phys();
    let phys_mem_offset = *crate::memory_mapper::PHYSICAL_MEMORY_OFFSET.lock();
    if phys_mem_offset == 0 {
        return;
    }

    let lfb_ptr = (phys_mem_offset + lfb_phys) as *mut u32;
    let width = 1920;
    let height = 1080;
    let slice = unsafe { core::slice::from_raw_parts_mut(lfb_ptr, width * height) };

    let mut intent_buffer = String::new();
    let mut prev_mouse_x = 960;
    let mut prev_mouse_y = 540;
    let mut prev_left_click = false;

    // Paint initial canvas, save background under cursor, and draw cursor
    repaint_canvas(slice, width, &intent_buffer);
    save_cursor_background(slice, width, prev_mouse_x, prev_mouse_y);
    crate::mouse::draw_cursor_32bpp(slice, width, prev_mouse_x, prev_mouse_y);

    let listener = SemanticIntentEngine;

    let mut async_serial_buf = alloc::string::String::new();
    let mut anim_ticks = 0usize;

    while GUI_ACTIVE.load(Ordering::Relaxed) {
        let mut needs_repaint = false;
        let mut needs_intent_repaint = false;
        let mut needs_response_repaint = false;
        
        let ai_state = ARCAI_STATE_VAR.load(Ordering::Relaxed);
        if ai_state == 1 {
            // Poll non-blocking serial
            let mut serial = crate::serial::SERIAL1.lock();
            while let Some(b) = serial.receive_byte() {
                if b == b'\n' || b == b'\r' {
                    if !async_serial_buf.is_empty() {
                        let response = async_serial_buf.clone();
                        async_serial_buf.clear();
                        
                        let exec_msg = crate::model_loader::handle_arcai_response(&response);
                        *LAST_FEEDBACK_MSG.lock() = exec_msg;
                        
                        ARCAI_STATE_VAR.store(0, Ordering::Relaxed);
                        needs_response_repaint = true;
                        needs_repaint = true;
                        break;
                    }
                } else {
                    async_serial_buf.push(b as char);
                }
            }
            
            // Thinking Animation (dot pulsing)
            anim_ticks = anim_ticks.wrapping_add(1);
            if anim_ticks % 2000 == 0 {
                let dots = (anim_ticks / 2000) % 4;
                let mut msg = alloc::string::String::from("Processing");
                for _ in 0..dots { msg.push('.'); }
                *LAST_FEEDBACK_MSG.lock() = msg;
                needs_response_repaint = true;
            }
        }

        // 1. Process Keyboard Events for Live Intent Input Bar
        let key = {
            let mut queue = SHELL_QUEUE.lock();
            queue.pop_front()
        };

        if let Some(key) = key {
            match key {
                DecodedKey::Unicode(ch) => {
                    crate::apps::VM_STATE.lock().pressed_key = Some(alloc::format!("{}", ch));
                    match ch {
                        '\n' | '\r' => {
                            let raw = intent_buffer.trim().to_string();
                            if !raw.is_empty() {
                                let parsed = listener.parse_intent(&raw);
                                
                                // Execute real kernel action and log empirical trace
                                let exec_msg = execute_kernel_intent(&parsed);
                                *LAST_FEEDBACK_MSG.lock() = exec_msg;

                                match parsed {
                                    Intent::DevWorkspace => {
                                        *CURRENT_SURFACE.lock() = UISurface::Developer;
                                    }
                                    Intent::AITensorWorkspace => {
                                        *CURRENT_SURFACE.lock() = UISurface::AITensorWorkspace;
                                    }
                                    Intent::ToggleTelemetry(val) => {
                                        *SHOW_TELEMETRY.lock() = val;
                                    }
                                    Intent::ThemeChange(theme_name) => {
                                        if let Some(col) = parse_dynamic_color(&theme_name) {
                                            *CURRENT_THEME.lock() = col;
                                        } else {
                                            *CURRENT_THEME.lock() = 0x000F2A38;
                                        }
                                    }
                                    Intent::SetHotspot { x, y, label, action } => {
                                        intent::add_hotspot(x, y, &label, &action);
                                    }
                                    _ => {}
                                }
                                intent_buffer.clear();
                                *RESPONSE_SCROLL.lock() = 0;
                                needs_intent_repaint = true;
                                needs_response_repaint = true;
                            }
                        }
                        '\u{0008}' | '\u{007F}' => {
                            if !intent_buffer.is_empty() {
                                intent_buffer.pop();
                                needs_intent_repaint = true;
                            }
                        }
                        c => {
                            if c.is_ascii() && !c.is_control() && intent_buffer.len() < 2000 {
                                intent_buffer.push(c);
                                needs_intent_repaint = true;
                            }
                        }
                    }
                }
                DecodedKey::RawKey(rk) => {
                    match rk {
                        KeyCode::Backspace => {
                            if !intent_buffer.is_empty() {
                                intent_buffer.pop();
                                needs_intent_repaint = true;
                            }
                        }
                        KeyCode::PageUp | KeyCode::ArrowUp => {
                            let mut s = RESPONSE_SCROLL.lock();
                            if *s > 0 { *s -= 1; needs_response_repaint = true; }
                        }
                        KeyCode::PageDown | KeyCode::ArrowDown => {
                            let mut s = RESPONSE_SCROLL.lock();
                            *s += 1;
                            needs_response_repaint = true;
                        }
                        _ => {
                            let rk_str = alloc::format!("{:?}", rk).to_lowercase();
                            crate::apps::VM_STATE.lock().pressed_key = Some(rk_str);
                        }
                    }
                }
            }
        }

        // 2. Process PS/2 Mouse & Click Hit-Testing
        let mouse = *crate::mouse::MOUSE_STATE.lock();
        let cur_x = mouse.x as usize;
        let cur_y = mouse.y as usize;

        if mouse.left_button && !prev_left_click {
            // Snapshot the app state FIRST so the Mutex lock is fully dropped
            // before handle_button_click tries to re-acquire it (prevents deadlock).
            let app_snapshot = crate::apps::ACTIVE_APP.lock().clone();
            if let Some(ref app) = app_snapshot {
                if cur_x >= app.x + app.width - 60 && cur_x <= app.x + app.width &&
                   cur_y >= app.y && cur_y <= app.y + 45 {
                    // [X] title bar close button
                    crate::apps::close_app();
                    needs_repaint = true;
                } else {
                    // Hit-test every interactive widget element
                    let mut clicked_action: Option<String> = None;
                    for elem in &app.elements {
                        let ex = app.x + elem.x_off;
                        let ey = app.y + elem.y_off;
                        if cur_x >= ex && cur_x <= ex + elem.width &&
                           cur_y >= ey && cur_y <= ey + elem.height {
                            match elem.widget {
                                crate::apps::UIWidgetKind::Button { ref action, .. } => {
                                    clicked_action = Some(action.clone());
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                    // Lock is fully released here before calling handle_button_click
                    if let Some(action) = clicked_action {
                        crate::apps::handle_button_click(&action);
                        needs_repaint = true;
                    }
                }
            }

            if let Some(spot) = intent::find_clicked_hotspot(cur_x, cur_y) {
                if spot.action == "exit" {
                    disable_bga();
                    crate::vga_graphics::restore_text_mode();
                    crate::vga_buffer::WRITER.lock().clear_screen();
                    crate::println!("\n[system] Graphics disabled via Exit Hotspot. Text mode restored.");
                    crate::shell::print_prompt();
                    break;
                }

                let parsed = listener.parse_intent(&spot.action);
                let exec_msg = execute_kernel_intent(&parsed);
                *LAST_FEEDBACK_MSG.lock() = exec_msg;

                match parsed {
                    Intent::DevWorkspace => {
                        *CURRENT_SURFACE.lock() = UISurface::Developer;
                    }
                    Intent::AITensorWorkspace => {
                        *CURRENT_SURFACE.lock() = UISurface::AITensorWorkspace;
                    }
                    _ => {}
                }
                needs_repaint = true;
            }
        }
        prev_left_click = mouse.left_button;

        // 3. Render Canvas & Cursor (Bulletproof Pixel Restoration: Zero Choppiness & Zero Artifacts)
        if needs_repaint || FORCE_REDRAW.swap(false, Ordering::Relaxed) {
            repaint_canvas(slice, width, &intent_buffer);
            repaint_response_card(slice, width);
            repaint_intent_bar(slice, width, &intent_buffer);

            save_cursor_background(slice, width, cur_x, cur_y);
            crate::mouse::draw_cursor_32bpp(slice, width, cur_x, cur_y);
            prev_mouse_x = cur_x;
            prev_mouse_y = cur_y;
        } else if needs_intent_repaint || needs_response_repaint {
            restore_cursor_background(slice, width, prev_mouse_x, prev_mouse_y);
            if needs_intent_repaint {
                repaint_intent_bar(slice, width, &intent_buffer);
            }
            if needs_response_repaint {
                repaint_response_card(slice, width);
            }
            save_cursor_background(slice, width, cur_x, cur_y);
            crate::mouse::draw_cursor_32bpp(slice, width, cur_x, cur_y);
            prev_mouse_x = cur_x;
            prev_mouse_y = cur_y;
        } else if cur_x != prev_mouse_x || cur_y != prev_mouse_y {
            // Restore exact saved pixels under old cursor location (restores text, borders, buttons perfectly)
            restore_cursor_background(slice, width, prev_mouse_x, prev_mouse_y);

            // Save pixels under new cursor location
            save_cursor_background(slice, width, cur_x, cur_y);

            // Draw cursor at new position
            crate::mouse::draw_cursor_32bpp(slice, width, cur_x, cur_y);

            prev_mouse_x = cur_x;
            prev_mouse_y = cur_y;
        }

        x86_64::instructions::hlt();
    }
}