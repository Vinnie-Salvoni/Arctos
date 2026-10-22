// =============================================================================
// Arctos OS — Memory Management
// =============================================================================
//
// This module provides virtual memory paging and frame allocation helpers.
// It maps the Level 4 Page Table and manages physical frame allocations.
// =============================================================================

use x86_64::structures::paging::mapper::TranslateResult;
use x86_64::structures::paging::OffsetPageTable;
use x86_64::{structures::paging::PageTable, PhysAddr, VirtAddr};

/// Returns a mutable reference to the active level 4 page table.
///
/// # Safety
/// The caller must guarantee that the complete physical memory is mapped to
/// virtual memory at the passed `physical_memory_offset`.
pub unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    use x86_64::registers::control::Cr3;

    // Read the CR3 register. The CPU uses the CR3 register to locate
    // the physical address of the Level 4 Page Table.
    let (level_4_table_frame, _) = Cr3::read();

    // Get the physical address of the Level 4 table
    let phys_addr = level_4_table_frame.start_address();

    // Convert the physical address to a virtual address by adding the offset
    let virt_addr = physical_memory_offset + phys_addr.as_u64();

    // Convert the raw virtual address pointer into a safe Rust mutable reference
    let page_table_ptr: *mut PageTable = virt_addr.as_mut_ptr();

    &mut *page_table_ptr // unsafe dereference
}

use bootloader::bootinfo::{MemoryMap, MemoryRegionType};
use x86_64::structures::paging::{FrameAllocator, PhysFrame, Size4KiB};

/// A FrameAllocator that returns usable frames from the bootloader's memory map.
pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryMap,
    next: usize,
}

impl BootInfoFrameAllocator {
    /// Create a FrameAllocator from the passed memory map.
    ///
    /// # Safety
    /// The caller must guarantee that all frames marked as `Usable` in the
    /// memory map are actually unused.
    pub unsafe fn init(memory_map: &'static MemoryMap) -> Self {
        BootInfoFrameAllocator {
            memory_map,
            next: 0, // Keeps track of how many frames we've allocated
        }
    }

    /// Returns an iterator over the usable frames described in the memory map.
    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        // 1. Get an iterator over all memory regions in the map
        let regions = self.memory_map.iter();

        // 2. Filter out the regions that are NOT usable (e.g. BIOS regions)
        let usable_regions = regions.filter(|r| r.region_type == MemoryRegionType::Usable);

        // 3. Convert the memory regions into start/end address ranges
        let addr_ranges = usable_regions.map(|r| r.range.start_addr()..r.range.end_addr());

        // 4. Split the address ranges into 4096-byte (4KB) steps (the size of a frame)
        let frame_addresses = addr_ranges.flat_map(|r| r.step_by(4096));

        // 5. Wrap each address in a safe `PhysFrame` type
        frame_addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }

    /// Calculates total physical memory size in bytes from the memory map.
    pub fn total_memory_bytes(&self) -> u64 {
        let mut total = 0;
        for region in self.memory_map.iter() {
            total += region.range.end_addr() - region.range.start_addr();
        }
        total
    }

    /// Calculates total usable physical memory size in bytes from the memory map.
    pub fn usable_memory_bytes(&self) -> u64 {
        let mut total = 0;
        for region in self.memory_map.iter() {
            if region.region_type == MemoryRegionType::Usable {
                total += region.range.end_addr() - region.range.start_addr();
            }
        }
        total
    }
}

// Implement the x86_64 library's FrameAllocator trait.
// This allows the CPU mapping code to ask our struct for a new frame.
unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        // Get the "nth" usable frame, incrementing our counter
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}

/// Initialize a new OffsetPageTable.
///
/// # Safety
/// The caller must guarantee that the complete physical memory is mapped to
/// virtual memory at the passed `physical_memory_offset`.
pub unsafe fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    // 1. Get a mutable reference to the Level 4 Page Table
    let level_4_table = active_level_4_table(physical_memory_offset);

    // 2. Initialize and return the OffsetPageTable wrapper
    OffsetPageTable::new(level_4_table, physical_memory_offset)
}

/// Translate a virtual address to its physical address using the
/// bootloader's identity-offset physical-memory mapping.
///
/// Returns `None` if the virtual address is not mapped.
///
/// # Safety
/// The caller must ensure `virtual_address` is a valid address in the
/// active address space and that the physical-memory offset is correct.
pub unsafe fn virtual_to_physical(
    virtual_address: VirtAddr,
    physical_memory_offset: VirtAddr,
) -> Option<PhysAddr> {
    use x86_64::structures::paging::Translate;

    let level_4_table = active_level_4_table(physical_memory_offset);
    let offset_page_table = OffsetPageTable::new(level_4_table, physical_memory_offset);

    match offset_page_table.translate(virtual_address) {
        TranslateResult::Mapped { frame, offset, .. } => {
            Some(frame.start_address() + u64::try_from(offset).ok()?)
        }
        TranslateResult::NotMapped => None,
        TranslateResult::InvalidFrameAddress(_) => None,
    }
}
