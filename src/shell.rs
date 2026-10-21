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

// Keep test allocations alive until the user runs `free`.
static ALLOCATED_CHUNKS: Mutex<Vec<Box<[u8]>>> = Mutex::new(Vec::new());

// Commands available to the shell and its autocomplete.
const COMMANDS: &[&str] = &[
    "help", "info", "clear", "echo", "print", "alloc", "free", "ls", "dir", "cat", "reboot",
    "restart", "shutdown", "poweroff",
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

        // Halt the CPU until the next interrupt.
        x86_64::instructions::hlt();
    }
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
            crate::println!("--- Arctos Shell Commands ---");
            crate::println!("help              - List commands");
            crate::println!("info              - Kernel and heap information");
            crate::println!("clear             - Clear the terminal");
            crate::println!("echo <text>       - Print text");
            crate::println!("alloc [bytes]     - Allocate a test memory block");
            crate::println!("free              - Release the last test allocation");
            crate::println!("ls                - List stored files");
            crate::println!("cat <filename>    - Read a stored file");
            crate::println!("reboot            - Restart the machine");
            crate::println!("shutdown          - Request machine shutdown");
        }

        "info" => {
            let stats = heap_stats();

            crate::println!("--- Arctos OS ---");
            crate::println!("Version: {}", env!("CARGO_PKG_VERSION"));
            crate::println!("Architecture: x86_64");
            crate::println!("Display: VGA text mode (80x25)");
            crate::println!("Kernel heap used: {} bytes", stats.0);
            crate::println!("Kernel heap free: {} bytes", stats.1);
        }

        "clear" => {
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

            {
                let chunks = ALLOCATED_CHUNKS.lock();

                if chunks.len() >= MAX_ACTIVE_ALLOCATIONS {
                    crate::println!("[error] Too many active test allocations. Run 'free' first.");
                    return;
                }
            }

            // Allocate memory and fill it with a known test pattern.
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

                None => {
                    crate::println!("[error] File '{}' not found.", args);
                }
            }
        }

        "reboot" | "restart" => {
            crate::println!("[system] Rebooting...");

            let mut port = x86_64::instructions::port::Port::<u8>::new(0x64);

            unsafe {
                port.write(0xFE);
            }

            // If the hardware does not reboot, remain halted.
            loop {
                x86_64::instructions::hlt();
            }
        }

        "shutdown" | "poweroff" => {
            crate::println!("[system] Requesting shutdown...");

            // Common QEMU/Bochs ACPI shutdown port.
            let mut acpi_port = x86_64::instructions::port::Port::<u16>::new(0x604);

            unsafe {
                acpi_port.write(0x2000);
            }

            // Some emulators use the Bochs shutdown port.
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
        // Shell input is restricted to ASCII, so byte lengths match characters.
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
