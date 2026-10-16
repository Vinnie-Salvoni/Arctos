// =============================================================================
// Arctos OS — Heap Allocator
// =============================================================================
//
// This module provides a global heap allocator for the kernel.
// It maps the virtual heap memory area and registers the heap allocator
// to enable standard dynamic memory allocation (Vec, String, Box).
// =============================================================================

use linked_list_allocator::LockedHeap;
use x86_64::structures::paging::{
    mapper::MapToError, FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB,
};
use x86_64::VirtAddr;

// 1. Register the global allocator with the Rust compiler
#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

// 2. Define where the heap lives in virtual memory and how big it is
pub const HEAP_START: usize = 0x_4444_4444_0000;
pub const HEAP_SIZE: usize = 16 * 1024 * 1024; // 16 MiB Heap for neural network matrices

/// Maps the heap pages in virtual memory and initializes the allocator.
pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    // 3. Convert start and end addresses into a page range
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + HEAP_SIZE as u64 - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    // 4. Loop through every page in the heap's range and map it to a physical frame
    for page in page_range {
        // Find a free physical frame of RAM
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
            
        // Set the page table flags: Present (usable) and Writable (read/write allowed)
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        
        // Map the virtual page to the physical frame
        unsafe {
            mapper.map_to(page, frame, flags, frame_allocator)?.flush();
        };
    }

    // 5. Initialize the allocator wrapper with our newly mapped virtual range
    unsafe {
        ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
    }

    Ok(())
}

/// Returns a tuple containing the (used, free) heap space in bytes.
pub fn heap_stats() -> (usize, usize) {
    let alloc = ALLOCATOR.lock();
    (alloc.used(), alloc.free())
}