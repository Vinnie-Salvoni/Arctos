// =============================================================================
// Arctos OS — Interrupt Handling & PIC Configuration
// =============================================================================
//
// This module sets up the Interrupt Descriptor Table (IDT) and configures the
// Intel 8259 Programmable Interrupt Controllers (PIC) to route hardware
// interrupts (such as the system timer and keyboard) to the CPU.
// =============================================================================

use crate::println;
use core::sync::atomic::{AtomicU64, Ordering};
use lazy_static::lazy_static;
use pic8259::ChainedPics;
use spin;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

// ── PIC Mapping ───────────────────────────────────────────────────────────
// Hardware interrupts (IRQs) by default overlap with CPU exceptions (0-31).
// To avoid conflicts, we map the primary PIC interrupts to indices 32-39
// and the secondary PIC interrupts to indices 40-47.
pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

//global timer tick counter
pub static TIMER_TICKS: AtomicU64 = AtomicU64::new(0);

/// Global controller for the primary and secondary PICs.
pub static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

/// Enum mapping PIC hardware interrupt indices to their assigned offsets.
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,        // IRQ 0 -> Vector 32 (System Timer)
    Keyboard = PIC_1_OFFSET + 1, // IRQ 1 -> Vector 33 (Keyboard Controller)
    Mouse = PIC_2_OFFSET + 4,    // IRQ 12 -> Vector 44 (PS/2 Mouse)
}

impl InterruptIndex {
    fn as_u8(self) -> u8 {
        self as u8
    }

    fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}

lazy_static! {
    /// Static Interrupt Descriptor Table (IDT) containing exception and hardware handlers.
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();

        // Register CPU Exception Handlers
        idt.breakpoint.set_handler_fn(breakpoint_handler);

        unsafe {
            idt.double_fault
                .set_handler_fn(double_fault_handler)
                .set_stack_index(crate::gdt::DOUBLE_FAULT_IST_INDEX);
        }

        idt.page_fault.set_handler_fn(page_fault_handler);

        // Register Hardware Interrupt Handlers
        idt[InterruptIndex::Timer.as_usize()].set_handler_fn(timer_handler);
        idt[InterruptIndex::Keyboard.as_usize()].set_handler_fn(keyboard_handler);
        idt[InterruptIndex::Mouse.as_usize()].set_handler_fn(mouse_handler);

        idt
    };
}

/// Loads the static IDT into the CPU's IDT register.
pub fn init_idt() {
    IDT.load();
}

// ── CPU Exception Handlers ────────────────────────────────────────────────

/// Handler for the Breakpoint Exception (int 3).
/// Safe, non-fatal exception used primarily for testing and debugging.
extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    println!("[interrupt] Exception: Breakpoint");
    println!("{:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: x86_64::structures::idt::PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;

    println!("EXCEPTION: PAGE FAULT");
    println!("Accessed Address: {:?}", Cr2::read());
    println!("Error Code: {:?}", error_code);
    println!("{:#?}", stack_frame);
    crate::halt_loop();
}

// ── Hardware Interrupt Handlers ───────────────────────────────────────────

/// Handler for the System Timer (IRQ 0 / Vector 32).
/// Fires periodically. We acknowledge the interrupt without screen spam.
extern "x86-interrupt" fn timer_handler(_stack_frame: InterruptStackFrame) {
    // We must explicitly notify the PIC that the interrupt has been handled (EOI),
    // otherwise the PIC will block all subsequent interrupts.
    unsafe {
        TIMER_TICKS.fetch_add(1, Ordering::Relaxed);
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
}

pub static SHELL_QUEUE: spin::Mutex<alloc::collections::VecDeque<pc_keyboard::DecodedKey>> =
    spin::Mutex::new(alloc::collections::VecDeque::new());

/// Handler for the Keyboard Controller (IRQ 1 / Vector 33).
/// Reads a scancode from I/O port 0x60 and translates it into Unicode characters.
extern "x86-interrupt" fn keyboard_handler(_stack_frame: InterruptStackFrame) {
    use pc_keyboard::{layouts, HandleControl, Keyboard, ScancodeSet1};
    use spin::Mutex;
    use x86_64::instructions::port::Port;

    // Static parser to track key state (e.g. holding shift, caps-lock status)
    lazy_static! {
        static ref KEYBOARD: Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> =
            Mutex::new(Keyboard::new(
                ScancodeSet1::new(),
                layouts::Us104Key,
                HandleControl::MapLettersToUnicode
            ));
    }

    let mut keyboard = KEYBOARD.lock();
    let mut port = Port::new(0x60);

    // Read raw hardware scancode from register
    let scancode: u8 = unsafe { port.read() };

    // Process scancode and push characters to queue
    if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
        if let Some(key) = keyboard.process_keyevent(key_event) {
            let mut queue = SHELL_QUEUE.lock();
            if queue.len() < 100 {
                // Max queue size to prevent unbounded memory growth
                queue.push_back(key);
            }
        }
    }

    // Acknowledge the keyboard interrupt with the PIC
    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}

/// Handler for the PS/2 Mouse Controller (IRQ 12 / Vector 44).
/// Reads bytes from port 0x60 and updates the global mouse state.
extern "x86-interrupt" fn mouse_handler(_stack_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;

    let mut port = Port::<u8>::new(0x60);
    let byte: u8 = unsafe { port.read() };

    crate::mouse::handle_mouse_byte(byte);

    // Acknowledge the mouse interrupt with the PIC (handles slave EOI)
    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Mouse.as_u8());
    }
}
