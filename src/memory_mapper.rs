// =============================================================================
// Arctos OS — Memory File Mapper (mmap)
// =============================================================================

use crate::fs;
use crate::memory::BootInfoFrameAllocator;
use spin::Mutex;
use x86_64::{
    structures::paging::{FrameAllocator, Mapper, OffsetPageTable, Page, PageTableFlags},
    VirtAddr,
};

// Thread-safe globals to allow the shell (which runs asynchronously after boot)
// to perform virtual page mapping.
pub static MAPPER: Mutex<Option<OffsetPageTable<'static>>> = Mutex::new(None);
pub static FRAME_ALLOCATOR: Mutex<Option<BootInfoFrameAllocator>> = Mutex::new(None);
pub static PHYSICAL_MEMORY_OFFSET: Mutex<u64> = Mutex::new(0);

/// Initializes the memory mapper context with the bootloader's active page table and allocator.
pub fn init(
    mapper: OffsetPageTable<'static>,
    frame_allocator: BootInfoFrameAllocator,
    phys_mem_offset: u64,
) {
    *MAPPER.lock() = Some(mapper);
    *FRAME_ALLOCATOR.lock() = Some(frame_allocator);
    *PHYSICAL_MEMORY_OFFSET.lock() = phys_mem_offset;
}

/// Returns the system memory metrics (total_bytes, usable_bytes).
pub fn system_memory_stats() -> Option<(u64, u64)> {
    let allocator_guard = FRAME_ALLOCATOR.lock();
    let allocator = allocator_guard.as_ref()?;
    Some((
        allocator.total_memory_bytes(),
        allocator.usable_memory_bytes(),
    ))
}

/// Maps a file from the secondary storage disk directly into virtual memory pages.
/// Returns the mapped virtual address and size in bytes.
pub fn map_file_to_virtual_addr(name: &str, start_addr: u64) -> Option<(u64, usize)> {
    // 1. Fetch file sector and size info from the filesystem
    let (data_start_sector, sector_count, file_size) = fs::get_file_info(name)?;

    let mut mapper_guard = MAPPER.lock();
    let mut allocator_guard = FRAME_ALLOCATOR.lock();
    let offset_guard = PHYSICAL_MEMORY_OFFSET.lock();

    let mapper = mapper_guard.as_mut()?;
    let allocator = allocator_guard.as_mut()?;
    let phys_mem_offset = *offset_guard;

    let size_bytes = file_size as usize;
    let page_count = (size_bytes + 4095) / 4096;
    let start_page = Page::containing_address(VirtAddr::new(start_addr));

    for i in 0..page_count {
        let page = start_page + i as u64;

        // 2. Allocate a physical frame for this page
        let frame = allocator.allocate_frame()?;

        // Get the virtual address representing this raw physical frame
        let frame_virt_addr = VirtAddr::new(phys_mem_offset + frame.start_address().as_u64());

        // Construct a safe mutable slice over the frame to write data
        let page_slice =
            unsafe { core::slice::from_raw_parts_mut(frame_virt_addr.as_mut_ptr::<u8>(), 4096) };

        // Zero out page contents first
        for byte in page_slice.iter_mut() {
            *byte = 0;
        }

        // 3. Read up to 8 sectors (4096 bytes) from disk into the page slice
        let page_sector_count = core::cmp::min(8, sector_count as usize - i * 8);
        let mut temp_buf = [0u16; 256];

        for s in 0..page_sector_count {
            let sector_idx = data_start_sector as usize + i * 8 + s;
            if crate::disk::read_sectors(sector_idx as u32, 1, &mut temp_buf).is_ok() {
                let dst_offset = s * 512;
                for w in 0..256 {
                    let bytes = temp_buf[w].to_le_bytes();
                    page_slice[dst_offset + w * 2] = bytes[0];
                    page_slice[dst_offset + w * 2 + 1] = bytes[1];
                }
            } else {
                return None; // Disk read failure
            }
        }

        // 4. Map the virtual page to our populated physical frame
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe {
            mapper.map_to(page, frame, flags, allocator).ok()?.flush();
        }
    }

    Some((start_addr, size_bytes))
}

/// Unmaps a range of virtual pages starting at start_addr.
pub fn unmap_virtual_addr(start_addr: u64, size_bytes: usize) -> Option<()> {
    let mut mapper_guard = MAPPER.lock();
    let mapper = mapper_guard.as_mut()?;

    let page_count = (size_bytes + 4095) / 4096;
    let start_page =
        Page::<x86_64::structures::paging::Size4KiB>::containing_address(VirtAddr::new(start_addr));

    for i in 0..page_count {
        let page = start_page + i as u64;
        if let Ok((_frame, flush)) = mapper.unmap(page) {
            flush.flush();
        }
    }
    Some(())
}
