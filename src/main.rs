// =============================================================================
// Arctos OS — Kernel Entry Point
// =============================================================================
//
// This is the bare-metal entry point for Arctos OS. The `bootloader` crate
// handles BIOS/UEFI bootstrap, switches the CPU to 64-bit long mode, sets up
// initial page tables, and then jumps here.
//
// We are running with:
//   - No standard library (no_std)
//   - No Rust runtime (no_main)
//   - Active Global Descriptor Table (GDT) & Task State Segment (TSS)
//   - Interrupt Descriptor Table (IDT) for exceptions & hardware interrupts
//   - OffsetPageTable virtual memory mapper
//   - Global Heap Allocator (linked_list_allocator) to enable `alloc` features
// =============================================================================

#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

// Bring in Rust's standard dynamic allocation library
extern crate alloc;

mod gdt;
mod vga_buffer;
mod interrupts;
mod memory;
mod allocator;
mod fs;
mod disk;
mod shell;
mod memory_mapper;
mod model_loader;
mod vga_graphics;
pub mod apps;
pub mod pit;
pub mod intent;
pub mod bga;
mod vga_font;
pub mod mouse;
pub mod user_profile;
pub mod serial;
pub mod ai;

use core::panic::PanicInfo;
use x86_64::VirtAddr;
use bootloader::{entry_point, BootInfo};
use alloc::boxed::Box;
use alloc::vec::Vec;

// Define kernel_main as the official bootloader entry point
entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    // 1. Clear the screen to a clean slate
    {
        let mut writer = vga_buffer::WRITER.lock();
        writer.clear_screen();
    }

    //call pit_init for timing accurately
    pit::init_pit();

    // 2. Initialize Core CPU Subsystems (Quietly first)
    gdt::init();
    interrupts::init_idt();
    unsafe { interrupts::PICS.lock().initialize(); }
    mouse::init_mouse();
    serial::init();
    x86_64::instructions::interrupts::enable();

    // 3. Initialize Virtual Memory & Heap Allocator
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe {
        memory::BootInfoFrameAllocator::init(&boot_info.memory_map)
    };
    allocator::init_heap(&mut mapper, &mut frame_allocator)
        .expect("heap initialization failed");

    // Initialize global memory mapper for shell and model loading
    memory_mapper::init(mapper, frame_allocator, boot_info.physical_memory_offset);

    // 4. Print Boot Banner (Visual Output Starts Here)
    {
        let mut writer = vga_buffer::WRITER.lock();
        writer.set_color(vga_buffer::Color::LightCyan, vga_buffer::Color::Black);
    }
    println!("    _                _              ___  ____  ");
    println!("   / \\   _ __ ___ | |_ ___  ___   / _ \\/ ___| ");
    println!("  / _ \\ | '__/ __|| __/ _ \\/ __| | | | \\___ \\ ");
    println!(" / ___ \\| | | (__ | || (_) \\__ \\ | |_| |___) |");
    println!("/_/   \\_\\_|  \\___| \\__\\___/|___/  \\___/|____/ ");
    println!();

    // 5. Print System Initialization Status
    {
        let mut writer = vga_buffer::WRITER.lock();
        writer.set_color(vga_buffer::Color::LightGreen, vga_buffer::Color::Black);
    }
    println!("[boot] Arctos OS v0.1.0");
    println!("[boot] Architecture: x86_64 (bare-metal)");
    println!("[boot] VGA text buffer initialized at 0xB8000");
    println!("[ok] GDT & TSS segment descriptors loaded.");
    println!("[ok] IDT interrupt handlers registered.");
    println!("[ok] Dual 8259 PIC controllers initialized.");
    println!("[ok] PS/2 Mouse Controller initialized (IRQ 12).");
    println!("[ok] Hardware interrupts enabled.");
    println!("[ok] Virtual memory paging mapping complete.");
    println!("[ok] Heap allocator initialized (100 KiB).");
    
    // Initialize AI Hardware Subsystem
    match ai::hardware::ArcHardwareContext::initialize() {
        Ok(_) => println!("[ok] Arc 140T Level Zero AI hardware initialized."),
        Err(e) => println!("[warn] AI hardware init failed: {}", e),
    }
    println!();

    // 6. Test Heap Allocations & Verify Memory Integrity
    {
        let mut writer = vga_buffer::WRITER.lock();
        writer.set_color(vga_buffer::Color::Yellow, vga_buffer::Color::Black);
    }
    
    // Allocate a Boxed array on the heap and verify the exact byte pattern
    let heap_value = Box::new([0x5Au8; 16]);
    for i in 0..16 {
        assert_eq!(heap_value[i], 0x5A, "Heap corruption detected on Box allocation!");
    }
    println!("[test] Boxed array allocated at: {:p} and verified pattern 0x5A.", heap_value.as_ptr());

    // Allocate a dynamic Vector on the heap and verify data retention
    let mut vec = Vec::new();
    for i in 0..256 {
        vec.push(i as u32);
    }
    for i in 0..256 {
        assert_eq!(vec[i], i as u32, "Heap corruption detected on Vector dynamic resize!");
    }
    println!("[test] Vector dynamically resized at: {:p} and verified 256 integers.", vec.as_slice().as_ptr());
    println!();

    {
        let mut writer = vga_buffer::WRITER.lock();
        writer.set_color(vga_buffer::Color::LightGreen, vga_buffer::Color::Black);
    }
    println!("[ok] Kernel bootstrap complete.");

    fs::mount();

    // Auto-load OS-level user profile preferences from AIVFS storage
    let prof = crate::user_profile::get_or_load_profile();
    *crate::bga::CURRENT_THEME.lock() = prof.theme_primary;
    *crate::bga::SECONDARY_THEME.lock() = prof.theme_secondary;
    *crate::bga::SHOW_TELEMETRY.lock() = prof.show_telemetry;
    match prof.active_surface.as_str() {
        "ai" | "tensor" => *crate::bga::CURRENT_SURFACE.lock() = crate::bga::UISurface::AITensorWorkspace,
        _ => *crate::bga::CURRENT_SURFACE.lock() = crate::bga::UISurface::Developer,
    }
    crate::user_profile::apply_profile_hotspots(&prof);

    // 8. Launch Interactive Command Shell
    shell::run();
}

/* ---------------------------------------------------------------------------
 Panic handler
 ---------------------------------------------------------------------------*/
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    crate::bga::disable_bga();
    vga_graphics::restore_text_mode();
    
    {
        let mut writer = vga_buffer::WRITER.lock();
        writer.set_color(vga_buffer::Color::LightRed, vga_buffer::Color::Black);
    }
    println!();
    println!("!!! KERNEL PANIC !!!");
    println!("{}", info);
    halt_loop();
}

/* ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------*/
pub fn halt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}
