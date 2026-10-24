/// The filesystem Superblock (Sector 0)
#[repr(C, packed)]
pub struct Superblock {
    pub magic: u32,               // Must be AIVFS magic (e.g., 0xA8C705)
    pub version: u16,             // Filesystem version
    pub total_sectors: u32,       // Size of disk in sectors
    pub inode_table_start: u32,   // Sector index where inode table begins (usually 1)
    pub inode_table_sectors: u32, // Number of sectors for inodes
    pub data_start_sector: u32,   // Sector index where files data begins
    pub label: [u8; 32],          // Partition volume label
}

/// The AIVFS Inode (64 bytes each, 8 per sector)
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct Inode {
    pub name: [u8; 32],         // Filename (31 chars + null termination)
    pub size: u32,              // Size of the file in bytes
    pub created_at: u64,        // Creation time (tick stamp)
    pub data_start_sector: u32, // Starting sector of file data blocks
    pub sector_count: u16,      // How many sectors this file consumes
    pub flags: u8,              // 0 = Unused, 1 = Regular File, 2 = Directory

    // ── AI Hooks ──
    pub semantic_tag_hash: u32, // Custom vector search category hash
    pub embedding_sector: u32,  // Pointer to sector storing file's AI embedding matrix

    pub _reserved: [u8; 5], // Padding to fit 64-byte size boundary (32+4+8+4+2+1+4+4+5 = 64)
}
