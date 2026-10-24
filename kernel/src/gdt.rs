// =============================================================================
// Arctos OS — Global Descriptor Table (GDT)
// =============================================================================
//
// Configures privilege levels (Ring 0/3) and the Task State Segment (TSS).
// Mandated by x86_64 CPU operation, even when paging is active.
// Includes a dedicated Interrupt Stack Table (IST) for double fault exceptions.
// =============================================================================

use lazy_static::lazy_static;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

lazy_static! {
    /// Static Task State Segment (TSS) containing exception handling stack pointers.
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            const STACK_SIZE: usize = 4096 * 5; // 20KB stack
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

            let stack_start = VirtAddr::from_ptr(core::ptr::addr_of!(STACK));
            let stack_end = stack_start + STACK_SIZE as u64;
            stack_end // Returns the top of the stack (since x86 stacks grow downwards)
        };
        tss
    };
}

/// Selector indices returned by the GDT registration.
struct Selectors {
    code_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}

lazy_static! {
    /// Static GDT containing code, data, and TSS segment descriptors.
    static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();
        let code_selector = gdt.add_entry(Descriptor::kernel_code_segment());
        let tss_selector = gdt.add_entry(Descriptor::tss_segment(&TSS));
        (gdt, Selectors { code_selector, tss_selector })
    };
}

/// Loads the static GDT and reloads the CS and TR segment registers.
pub fn init() {
    use x86_64::instructions::segmentation::{Segment, CS};
    use x86_64::instructions::tables::load_tss;

    // Load the GDT into the GDTR CPU register
    GDT.0.load();

    // Reload segment registers to reference GDT descriptors
    unsafe {
        CS::set_reg(GDT.1.code_selector);
        load_tss(GDT.1.tss_selector);
    }
}
