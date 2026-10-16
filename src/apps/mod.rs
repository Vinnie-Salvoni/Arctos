// =============================================================================
// Arctos OS — Standalone Application Execution & Loader
// =============================================================================

#![allow(improper_ctypes_definitions)]



use crate::vga_buffer::WRITER;
use pc_keyboard::DecodedKey;

#[repr(C, packed)]
pub struct ArcHeader {
    pub magic: [u8; 4],        // b"ARCS"
    pub entry_offset: u32,     // Offset of entry function inside code block
    pub code_size: u32,        // Size of raw code block
}

#[repr(C)]
pub struct KernelApi {
    pub draw_char: extern "C" fn(usize, usize, u32, u8),
    pub draw_rect: extern "C" fn(usize, usize, usize, usize, u8),
    pub draw_rect_filled: extern "C" fn(usize, usize, usize, usize, u8),
    pub clear_screen: extern "C" fn(u8),
    pub swap_buffers: extern "C" fn(),
    pub read_key: extern "C" fn() -> u32,
    pub write_file: extern "C" fn(*const u8, usize, *const u8, usize) -> bool,
    pub read_file: extern "C" fn(*const u8, usize, *mut u8, usize) -> usize,
    pub sleep: extern "C" fn(usize),
    pub get_cmdline: extern "C" fn(*mut u8, usize) -> usize,
    pub list_files: extern "C" fn(*mut u8, usize) -> usize,
    pub delete_file: extern "C" fn(*const u8, usize) -> bool,
    pub get_ticks: extern "C" fn() -> u64,
   
}

// =============================================================================
// API Wrapper Functions (extern "C" interface)
// =============================================================================

extern "C" fn api_get_ticks() -> u64 {
        crate::interrupts::TIMER_TICKS.load(core::sync::atomic::Ordering::Relaxed)
}

extern "C" fn api_draw_char(x: usize, y: usize, c: u32, color: u8) {
    if let Some(ch) = core::char::from_u32(c) {
        crate::vga_graphics::draw_char(x, y, ch, color);
    }
}

extern "C" fn api_draw_rect(x: usize, y: usize, w: usize, h: usize, color: u8) {
    crate::vga_graphics::draw_rect(x, y, w, h, color);
}

extern "C" fn api_draw_rect_filled(x: usize, y: usize, w: usize, h: usize, color: u8) {
    crate::vga_graphics::draw_rect_filled(x, y, w, h, color);
}

extern "C" fn api_clear_screen(color: u8) {
    crate::vga_graphics::clear_screen(color);
}

extern "C" fn api_swap_buffers() {
    crate::vga_graphics::swap_buffers();
}

extern "C" fn api_read_key() -> u32 {
    let key = x86_64::instructions::interrupts::without_interrupts(|| {
        let mut queue = crate::interrupts::SHELL_QUEUE.lock();
        queue.pop_front()
    });

    if let Some(key) = key {
        match key {
            DecodedKey::Unicode(c) => {
                if c == '\u{001B}' {
                    0x0002_0001 // Escape
                } else if c == '\u{0008}' {
                    0x0002_0003 // Backspace
                } else if c == '\n' || c == '\r' {
                    0x0002_0004 // Enter/Newline
                } else {
                    0x0001_0000 | (c as u32)
                }
            }
            DecodedKey::RawKey(raw) => {
                use pc_keyboard::KeyCode;
                match raw {
                    KeyCode::Escape => 0x0002_0001,
                    KeyCode::F10 => 0x0002_0002,
                    KeyCode::ArrowUp => 0x0002_0005,
                    KeyCode::ArrowDown => 0x0002_0006,
                    KeyCode::ArrowLeft => 0x0002_0007,
                    KeyCode::ArrowRight => 0x0002_0008,
                    KeyCode::F1 => 0x0002_0009,
                    _ => 0,
                }
            }
        }
    } else {
        0
    }
}

extern "C" fn api_write_file(name_ptr: *const u8, name_len: usize, data_ptr: *const u8, data_len: usize) -> bool {
    let name_slice = unsafe { core::slice::from_raw_parts(name_ptr, name_len) };
    let data_slice = unsafe { core::slice::from_raw_parts(data_ptr, data_len) };
    if let Ok(name_str) = core::str::from_utf8(name_slice) {
        crate::fs::write_file(name_str, data_slice)
    } else {
        false
    }
}

extern "C" fn api_read_file(name_ptr: *const u8, name_len: usize, buffer_ptr: *mut u8, buffer_len: usize) -> usize {
    let name_slice = unsafe { core::slice::from_raw_parts(name_ptr, name_len) };
    if let Ok(name_str) = core::str::from_utf8(name_slice) {
        if let Some(data) = crate::fs::read_file(name_str) {
            let read_len = core::cmp::min(data.len(), buffer_len);
            let dst_slice = unsafe { core::slice::from_raw_parts_mut(buffer_ptr, buffer_len) };
            dst_slice[..read_len].copy_from_slice(&data[..read_len]);
            read_len
        } else {
            0
        }
    } else {
        0
    }
}

extern "C" fn api_sleep(cycles: usize) {
    for _ in 0..cycles {
        x86_64::instructions::nop();
    }
}

pub static APP_ARGUMENT: spin::Mutex<alloc::string::String> = spin::Mutex::new(alloc::string::String::new());

extern "C" fn api_get_cmdline(buffer_ptr: *mut u8, buffer_len: usize) -> usize {
    let arg = APP_ARGUMENT.lock();
    let bytes = arg.as_bytes();
    let read_len = core::cmp::min(bytes.len(), buffer_len);
    if read_len > 0 {
        let dst_slice = unsafe { core::slice::from_raw_parts_mut(buffer_ptr, buffer_len) };
        dst_slice[..read_len].copy_from_slice(&bytes[..read_len]);
    }
    read_len
}

extern "C" fn api_list_files(buffer_ptr: *mut u8, buffer_len: usize) -> usize {
    let files = crate::fs::list_files();
    let mut offset = 0;
    let dst_slice = unsafe { core::slice::from_raw_parts_mut(buffer_ptr, buffer_len) };
    
    for f in files {
        let name_bytes = f.as_bytes();
        if offset + name_bytes.len() + 1 > buffer_len {
            break;
        }
        dst_slice[offset..offset + name_bytes.len()].copy_from_slice(name_bytes);
        offset += name_bytes.len();
        dst_slice[offset] = 0; // Null separator
        offset += 1;
    }
    if offset < buffer_len {
        dst_slice[offset] = 0;
        offset += 1;
    }
    offset
}

extern "C" fn api_delete_file(name_ptr: *const u8, name_len: usize) -> bool {
    let name_slice = unsafe { core::slice::from_raw_parts(name_ptr, name_len) };
    if let Ok(name_str) = core::str::from_utf8(name_slice) {
        crate::fs::delete_file(name_str)
    } else {
        false
    }
}

// =============================================================================
// Application Loader
// =============================================================================

pub fn run_app(filename: &str) -> bool {

   
    // 1. Read binary application file from AIVFS storage
    let app_bytes = match crate::fs::read_file(filename) {
        Some(bytes) => bytes,
        None => {
            crate::println!("[error] Executable file '{}' not found.", filename);
            return false;
        }
    };

    if app_bytes.len() < core::mem::size_of::<ArcHeader>() {
        crate::println!("[error] Executable file '{}' is too small to be valid.", filename);
        return false;
    }

    // Parse the executable header at the start of the loaded bytes
    let header = unsafe { &*(app_bytes.as_ptr() as *const ArcHeader) };
    if header.magic != *b"ARCS" {
        crate::println!("[error] Invalid executable magic signature: {:?}", header.magic);
        return false;
    }

    let target_addr: u64 = 0x4000_0000_0000;
    
    // 2. Map isolated virtual memory pages for the application
    if crate::memory_mapper::map_file_to_virtual_addr(filename, target_addr).is_none() {
        crate::println!("[error] Memory allocation failed for application mapping.");
        return false;
    }

    // 3. Populate Kernel API structure
    let api = KernelApi {
        draw_char: api_draw_char,
        draw_rect: api_draw_rect,
        draw_rect_filled: api_draw_rect_filled,
        clear_screen: api_clear_screen,
        swap_buffers: api_swap_buffers,
        read_key: api_read_key,
        write_file: api_write_file,
        read_file: api_read_file,
        sleep: api_sleep,
        get_cmdline: api_get_cmdline,
        list_files: api_list_files,
        delete_file: api_delete_file,
        get_ticks: api_get_ticks,
    };

    // 4. Initialize display controllers and execute application entry
    let phys_mem_offset = {
        let offset_guard = crate::memory_mapper::PHYSICAL_MEMORY_OFFSET.lock();
        *offset_guard
    };

    // Initialize Mode 13h graphics screen
    crate::vga_graphics::init_graphics(phys_mem_offset);

    // Calculate exact entry point address offset by the header size
    let entry_addr = target_addr + core::mem::size_of::<ArcHeader>() as u64 + header.entry_offset as u64;

    // Cast the entry address to our application function pointer signature
    type EntryFn = extern "C" fn(&KernelApi);
    let entry: EntryFn = unsafe { core::mem::transmute(entry_addr) };

    // Jump instruction pointer to execute the standalone application!
    entry(&api);

    // 5. Clean exit, unmap memory, and return display to terminal text mode
    crate::vga_graphics::restore_text_mode();
    {
        let mut writer = WRITER.lock();
        writer.clear_screen();
    }

    crate::memory_mapper::unmap_virtual_addr(target_addr, app_bytes.len());
    true
}

// =============================================================================
// Dynamic AI Component Renderer & Interactive App Engine
// Zero hardcoded apps — renders dynamic UI element trees generated live by AI.
// =============================================================================

use spin::Mutex;
use alloc::string::String;
use alloc::vec::Vec;

// =============================================================================
// Dynamic Widget Types
// =============================================================================

#[derive(Debug, Clone)]
pub enum UIWidgetKind {
    Panel     { bg_color: u32, border_color: u32 },
    Text      { text: String, color: u32 },
    Button    { label: String, action: String, bg_color: u32, text_color: u32 },
    DisplayScreen { text: String, bg_color: u32, text_color: u32 },
    Slider    { value: f32, min: f32, max: f32, label: String, bg_color: u32, accent_color: u32 },
    ProgressBar { progress: f32, label: String, bg_color: u32, fill_color: u32 },
    /// AI-generated program canvas — runs VM bytecode every frame.
    ProgramCanvas { program: Vec<VmOp> },
}

// =============================================================================
// ArcAI Micro-VM  (no hardcoded games — the AI writes all programs)
// =============================================================================
// The AI generates a list of VmOps in the wire spec.  The kernel executes them
// every frame.  This gives the AI the power to program arbitrary animated apps.
//
// Wire format for one op (semicolon-separated fields inside PROGRAM=...;):
//   OP:fill_rect,X,Y,W,H,COLOR
//   OP:draw_text,X,Y,"text",COLOR
//   OP:draw_cell,CX,CY,CW,CH,COLOR
//   OP:set_var,NAME,VALUE
//   OP:add_var,NAME,DELTA
//   OP:mod_var,NAME,MODULUS
//   OP:if_eq,NAME,VALUE,THEN_OP_INDEX
//   OP:jump,OP_INDEX
//   OP:on_key,KEY,OP_INDEX    (KEY: up/down/left/right/r/space/enter)
//
// State variables are named strings mapped to i64 values.
// Programs run every render frame — use TICK_RATE=N to pace updates.

#[derive(Debug, Clone)]
pub enum VmOp {
    FillRect  { x: i64, y: i64, w: i64, h: i64, color: u32 },
    DrawText  { x: i64, y: i64, text: String, color: u32 },
    DrawCell  { cx: i64, cy: i64, cw: i64, ch: i64, color: u32 },
    SetVar    { name: String, value: i64 },
    AddVar    { name: String, delta: i64 },
    ModVar    { name: String, modulus: i64 },
    IfEq      { name: String, value: i64, then_idx: usize },
    Jump      { idx: usize },
    OnKey     { key: String, then_idx: usize },
    TickGate  { every_n: u64 },  // skip following ops if tick % every_n != 0
    Halt,
}

/// Parse a single OP:... wire string into a VmOp.
fn parse_vm_op(s: &str) -> Option<VmOp> {
    let fields: Vec<&str> = s.splitn(10, ',').collect();
    if fields.is_empty() { return None; }
    match fields[0].trim() {
        "fill_rect" if fields.len() >= 6 => Some(VmOp::FillRect {
            x: fields[1].trim().parse().ok()?,
            y: fields[2].trim().parse().ok()?,
            w: fields[3].trim().parse().ok()?,
            h: fields[4].trim().parse().ok()?,
            color: parse_vm_color(fields[5].trim()),
        }),
        "draw_text" if fields.len() >= 5 => Some(VmOp::DrawText {
            x: fields[1].trim().parse().ok()?,
            y: fields[2].trim().parse().ok()?,
            text: String::from(fields[3].trim().trim_matches('"')),
            color: parse_vm_color(fields[4].trim()),
        }),
        "draw_cell" if fields.len() >= 6 => Some(VmOp::DrawCell {
            cx: fields[1].trim().parse().ok()?,
            cy: fields[2].trim().parse().ok()?,
            cw: fields[3].trim().parse().ok()?,
            ch: fields[4].trim().parse().ok()?,
            color: parse_vm_color(fields[5].trim()),
        }),
        "set_var" if fields.len() >= 3 => Some(VmOp::SetVar {
            name: String::from(fields[1].trim()),
            value: fields[2].trim().parse().unwrap_or(0),
        }),
        "add_var" if fields.len() >= 3 => Some(VmOp::AddVar {
            name: String::from(fields[1].trim()),
            delta: fields[2].trim().parse().unwrap_or(0),
        }),
        "mod_var" if fields.len() >= 3 => Some(VmOp::ModVar {
            name: String::from(fields[1].trim()),
            modulus: fields[3].trim().parse().unwrap_or(1),
        }),
        "if_eq" if fields.len() >= 4 => Some(VmOp::IfEq {
            name: String::from(fields[1].trim()),
            value: fields[2].trim().parse().unwrap_or(0),
            then_idx: fields[3].trim().parse().unwrap_or(0),
        }),
        "jump" if fields.len() >= 2 => Some(VmOp::Jump {
            idx: fields[1].trim().parse().unwrap_or(0),
        }),
        "on_key" if fields.len() >= 3 => Some(VmOp::OnKey {
            key: String::from(fields[1].trim()),
            then_idx: fields[2].trim().parse().unwrap_or(0),
        }),
        "tick_gate" if fields.len() >= 2 => Some(VmOp::TickGate {
            every_n: fields[1].trim().parse().unwrap_or(1),
        }),
        "halt" => Some(VmOp::Halt),
        _ => None,
    }
}

fn parse_vm_color(s: &str) -> u32 {
    if s.starts_with("0x") || s.starts_with("0X") {
        u32::from_str_radix(&s[2..], 16).unwrap_or(0xFF0C1A2B)
    } else {
        crate::bga::parse_dynamic_color(s).unwrap_or(0xFF0C1A2B)
    }
}

/// Parse a full PROGRAM wire segment into a Vec<VmOp>.
/// Wire format: OP:fill_rect,...;OP:set_var,...;...
pub fn parse_program_wire(wire: &str) -> Vec<VmOp> {
    let mut ops = Vec::new();
    for segment in wire.split(';') {
        let seg = segment.trim();
        if !seg.starts_with("OP:") { continue; }
        if let Some(op) = parse_vm_op(&seg[3..]) {
            ops.push(op);
        }
    }
    ops
}

// =============================================================================
// VM Runtime State  (shared between render frames)
// =============================================================================

use alloc::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct VmState {
    pub vars: BTreeMap<String, i64>,
    pub tick: u64,
    pub pressed_key: Option<String>,
}

pub static VM_STATE: Mutex<VmState> = Mutex::new(VmState {
    vars: BTreeMap::new(),
    tick: 0,
    pressed_key: None,
});

// =============================================================================
// App Window
// =============================================================================

#[derive(Debug, Clone)]
pub struct DynamicAppWindow {
    pub title: String,
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    pub state_var: String,
    pub elements: Vec<UIElement>,
}

pub static ACTIVE_APP: Mutex<Option<DynamicAppWindow>> = Mutex::new(None);

pub fn spawn_dynamic_app(title: &str, initial_state: &str, elements: Vec<UIElement>) {
    // Reset the VM state for the new app
    let mut vm = VM_STATE.lock();
    vm.vars.clear();
    vm.tick = 0;
    vm.pressed_key = None;
    drop(vm);

    let app = DynamicAppWindow {
        title: String::from(if title.is_empty() { "AI App" } else { title }),
        x: 0, y: 0, width: 1920, height: 1080,
        state_var: String::from(if initial_state.is_empty() { "0" } else { initial_state }),
        elements,
    };
    *ACTIVE_APP.lock() = Some(app);
}

pub fn parse_and_spawn_app(spec: &str) -> Option<String> {
    let mut title = String::from("AI App");
    let mut elements = Vec::new();

    for token in spec.split(';') {
        let tok = token.trim();
        if tok.starts_with("TITLE=") {
            title = String::from(&tok[6..]);
        } else if tok.starts_with("WIDGET=") {
            let parts: Vec<&str> = tok[7..].split(',').collect();
            if parts.len() >= 5 {
                let kind = parts[0].trim();
                let x_off = parts[1].trim().parse().unwrap_or(40);
                let y_off = parts[2].trim().parse().unwrap_or(60);
                let width = parts[3].trim().parse().unwrap_or(160);
                let height = parts[4].trim().parse().unwrap_or(60);
                let label = if parts.len() > 5 { parts[5].trim() } else { "?" };
                let action = if parts.len() > 6 { parts[6].trim() } else { "btn:?" };
                let color_str = if parts.len() > 7 { parts[7].trim() } else { "0xFF0C1A2B" };
                let color = parse_vm_color(color_str);

                let widget = match kind {
                    "button" => UIWidgetKind::Button {
                        label: String::from(label),
                        action: String::from(action),
                        bg_color: color,
                        text_color: 0x00FFFFFF,
                    },
                    "display" => UIWidgetKind::DisplayScreen {
                        text: String::from(label),
                        bg_color: color,
                        text_color: 0x0000FFCC,
                    },
                    "text" => UIWidgetKind::Text {
                        text: String::from(label),
                        color,
                    },
                    _ => UIWidgetKind::Panel {
                        bg_color: color,
                        border_color: 0x0000E5FF,
                    },
                };

                elements.push(UIElement {
                    x_off,
                    y_off,
                    width,
                    height,
                    widget,
                });
            }
        }
    }

    if !elements.is_empty() {
        spawn_dynamic_app(&title, "0", elements);
        Some(title)
    } else {
        None
    }
}

#[derive(Debug, Clone)]
pub struct UIElement {
    pub x_off: usize,
    pub y_off: usize,
    pub width: usize,
    pub height: usize,
    pub widget: UIWidgetKind,
}

pub fn close_app() {
    *ACTIVE_APP.lock() = None;
    crate::bga::FORCE_REDRAW.store(false, core::sync::atomic::Ordering::Relaxed);
}

pub fn handle_button_click(action: &str) {
    let mut app_guard = ACTIVE_APP.lock();
    if let Some(ref mut app) = *app_guard {
        if action == "btn:C" {
            app.state_var = String::from("0");
        } else if action == "btn:=" {
            // Evaluate simple arithmetic expression in state_var
            let expr = app.state_var.clone();
            let result = eval_arithmetic(&expr);
            app.state_var = result;
        } else if action.starts_with("btn:") {
            let val = &action[4..];
            if app.state_var == "0" || app.state_var == "0.0" {
                app.state_var = String::from(val);
            } else {
                app.state_var.push_str(val);
            }
        }

        // Update display screens with new state_var
        let current_state = app.state_var.clone();
        for elem in app.elements.iter_mut() {
            if let UIWidgetKind::DisplayScreen { ref mut text, .. } = elem.widget {
                *text = current_state.clone();
            }
        }
    }
}

fn eval_arithmetic(expr: &str) -> String {
    if expr.contains('+') {
        let parts: Vec<&str> = expr.split('+').collect();
        if parts.len() == 2 {
            let a: f64 = parts[0].trim().parse().unwrap_or(0.0);
            let b: f64 = parts[1].trim().parse().unwrap_or(0.0);
            return alloc::format!("{}", a + b);
        }
    } else if expr.contains('-') {
        let parts: Vec<&str> = expr.split('-').collect();
        if parts.len() == 2 {
            let a: f64 = parts[0].trim().parse().unwrap_or(0.0);
            let b: f64 = parts[1].trim().parse().unwrap_or(0.0);
            return alloc::format!("{}", a - b);
        }
    } else if expr.contains('*') {
        let parts: Vec<&str> = expr.split('*').collect();
        if parts.len() == 2 {
            let a: f64 = parts[0].trim().parse().unwrap_or(0.0);
            let b: f64 = parts[1].trim().parse().unwrap_or(0.0);
            return alloc::format!("{}", a * b);
        }
    } else if expr.contains('/') {
        let parts: Vec<&str> = expr.split('/').collect();
        if parts.len() == 2 {
            let a: f64 = parts[0].trim().parse().unwrap_or(0.0);
            let b: f64 = parts[1].trim().parse().unwrap_or(1.0);
            if b != 0.0 {
                return alloc::format!("{}", a / b);
            }
        }
    }
    String::from(expr)
}
