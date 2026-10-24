pub mod structures;

use alloc::string::String;
use alloc::vec::Vec;
use structures::{Inode, Superblock};

const AIVFS_MAGIC: u32 = 0xA8C705;
const AIVFS_VERSION: u16 = 1;
const SECTOR_SIZE: usize = 512;
const TOTAL_SECTORS: usize = 20480;
const SUPERBLOCK_SECTOR: usize = 0;
const INODE_TABLE_START: usize = 1;
const INODE_TABLE_SECTORS: usize = 7;
const INODES_PER_SECTOR: usize = 8;
const MAX_INODES: usize = INODE_TABLE_SECTORS * INODES_PER_SECTOR;
const DATA_START_SECTOR: usize = 8;
const FLAG_UNUSED: u8 = 0;
const FLAG_FILE: u8 = 1;

/// Return a filename only when its stored representation is valid.
fn inode_name(node: &Inode) -> Option<&str> {
    let end = node.name.iter().position(|&byte| byte == 0)?;

    if end == 0 || node.name[end + 1..].iter().any(|&byte| byte != 0) {
        return None;
    }

    core::str::from_utf8(&node.name[..end]).ok()
}

fn sectors_needed(bytes: usize) -> u16 {
    let sectors = bytes / SECTOR_SIZE + usize::from(bytes % SECTOR_SIZE != 0);
    sectors as u16
}

/// Read one sector from the physical disk using the ATA PIO driver.
fn read_sector(sector: usize, out: &mut [u8; SECTOR_SIZE]) -> bool {
    if sector >= TOTAL_SECTORS {
        return false;
    }

    let mut temp = [0u16; 256];

    if crate::disk::read_sectors(sector as u32, 1, &mut temp).is_err() {
        return false;
    }

    for i in 0..256 {
        let bytes = temp[i].to_le_bytes();
        out[i * 2] = bytes[0];
        out[i * 2 + 1] = bytes[1];
    }

    true
}

/// Write one sector to the physical disk using the ATA PIO driver.
fn write_sector(sector: usize, bytes: &[u8; SECTOR_SIZE]) -> bool {
    if sector >= TOTAL_SECTORS {
        return false;
    }

    let mut temp = [0u16; 256];

    for i in 0..256 {
        temp[i] = u16::from_le_bytes([bytes[i * 2], bytes[i * 2 + 1]]);
    }

    crate::disk::write_sectors(sector as u32, 1, &temp).is_ok()
}

fn read_superblock() -> Option<Superblock> {
    let mut sector = [0u8; SECTOR_SIZE];

    if !read_sector(SUPERBLOCK_SECTOR, &mut sector) {
        return None;
    }

    let magic = u32::from_le_bytes(sector[0..4].try_into().ok()?);
    let version = u16::from_le_bytes(sector[4..6].try_into().ok()?);
    let total_sectors = u32::from_le_bytes(sector[6..10].try_into().ok()?);
    let inode_table_start = u32::from_le_bytes(sector[10..14].try_into().ok()?);
    let inode_table_sectors = u32::from_le_bytes(sector[14..18].try_into().ok()?);
    let data_start_sector = u32::from_le_bytes(sector[18..22].try_into().ok()?);

    let mut label = [0u8; 32];
    label.copy_from_slice(&sector[22..54]);

    Some(Superblock {
        magic,
        version,
        total_sectors,
        inode_table_start,
        inode_table_sectors,
        data_start_sector,
        label,
    })
}

fn write_superblock(sb: &Superblock) -> bool {
    let mut sector = [0u8; SECTOR_SIZE];

    sector[0..4].copy_from_slice(&sb.magic.to_le_bytes());
    sector[4..6].copy_from_slice(&sb.version.to_le_bytes());
    sector[6..10].copy_from_slice(&sb.total_sectors.to_le_bytes());
    sector[10..14].copy_from_slice(&sb.inode_table_start.to_le_bytes());
    sector[14..18].copy_from_slice(&sb.inode_table_sectors.to_le_bytes());
    sector[18..22].copy_from_slice(&sb.data_start_sector.to_le_bytes());
    sector[22..54].copy_from_slice(&sb.label);

    write_sector(SUPERBLOCK_SECTOR, &sector)
}

/// Read the complete inode table.
pub fn read_inode_table() -> Option<Vec<Inode>> {
    let mut inodes = Vec::with_capacity(MAX_INODES);

    for s in 0..INODE_TABLE_SECTORS {
        let mut sector = [0u8; SECTOR_SIZE];

        if !read_sector(INODE_TABLE_START + s, &mut sector) {
            return None;
        }

        for i in 0..INODES_PER_SECTOR {
            let off = i * 64;

            let mut name = [0u8; 32];
            name.copy_from_slice(&sector[off..off + 32]);

            let size = u32::from_le_bytes(sector[off + 32..off + 36].try_into().ok()?);
            let created_at = u64::from_le_bytes(sector[off + 36..off + 44].try_into().ok()?);
            let data_start_sector = u32::from_le_bytes(sector[off + 44..off + 48].try_into().ok()?);
            let sector_count = u16::from_le_bytes(sector[off + 48..off + 50].try_into().ok()?);
            let flags = sector[off + 50];
            let semantic_tag_hash = u32::from_le_bytes(sector[off + 51..off + 55].try_into().ok()?);
            let embedding_sector = u32::from_le_bytes(sector[off + 55..off + 59].try_into().ok()?);

            let mut reserved = [0u8; 5];
            reserved.copy_from_slice(&sector[off + 59..off + 64]);

            inodes.push(Inode {
                name,
                size,
                created_at,
                data_start_sector,
                sector_count,
                flags,
                semantic_tag_hash,
                embedding_sector,
                _reserved: reserved,
            });
        }
    }

    Some(inodes)
}

/// Write the complete inode table.
fn write_inode_table(inodes: &[Inode]) -> bool {
    if inodes.len() != MAX_INODES {
        return false;
    }

    for s in 0..INODE_TABLE_SECTORS {
        let mut sector = [0u8; SECTOR_SIZE];

        for i in 0..INODES_PER_SECTOR {
            let idx = s * INODES_PER_SECTOR + i;
            let off = i * 64;
            let node = &inodes[idx];

            sector[off..off + 32].copy_from_slice(&node.name);
            sector[off + 32..off + 36].copy_from_slice(&node.size.to_le_bytes());
            sector[off + 36..off + 44].copy_from_slice(&node.created_at.to_le_bytes());
            sector[off + 44..off + 48].copy_from_slice(&node.data_start_sector.to_le_bytes());
            sector[off + 48..off + 50].copy_from_slice(&node.sector_count.to_le_bytes());
            sector[off + 50] = node.flags;
            sector[off + 51..off + 55].copy_from_slice(&node.semantic_tag_hash.to_le_bytes());
            sector[off + 55..off + 59].copy_from_slice(&node.embedding_sector.to_le_bytes());
            sector[off + 59..off + 64].copy_from_slice(&node._reserved);
        }

        if !write_sector(INODE_TABLE_START + s, &sector) {
            return false;
        }
    }

    true
}

/// Validate file extents and return the first sector after all allocated data.
///
/// This filesystem currently uses append-only allocation for file updates.
fn find_free_data_sector(inodes: &[Inode]) -> Option<u32> {
    let mut next = DATA_START_SECTOR as u32;

    // Validate active inode metadata and disk ranges.
    for node in inodes {
        if node.flags == FLAG_UNUSED {
            continue;
        }

        if node.flags != FLAG_FILE {
            return None;
        }

        // Reject malformed stored filenames.
        inode_name(node)?;

        let start = node.data_start_sector;
        let count = u32::from(node.sector_count);

        if start < DATA_START_SECTOR as u32 || start > TOTAL_SECTORS as u32 {
            return None;
        }

        let end = start.checked_add(count)?;

        if end > TOTAL_SECTORS as u32 {
            return None;
        }

        let capacity = (count as usize).checked_mul(SECTOR_SIZE)?;

        if node.size as usize > capacity {
            return None;
        }

        if end > next {
            next = end;
        }
    }

    // Reject overlapping allocated ranges.
    for i in 0..inodes.len() {
        let a = &inodes[i];

        if a.flags != FLAG_FILE || a.sector_count == 0 {
            continue;
        }

        let a_start = a.data_start_sector;
        let a_end = a_start.checked_add(u32::from(a.sector_count))?;

        for b in &inodes[..i] {
            if b.flags != FLAG_FILE || b.sector_count == 0 {
                continue;
            }

            let b_start = b.data_start_sector;
            let b_end = b_start.checked_add(u32::from(b.sector_count))?;

            if a_start < b_end && b_start < a_end {
                return None;
            }
        }
    }

    Some(next)
}

fn write_data_sectors(start_sector: u32, sector_count: u16, data: &[u8]) -> bool {
    let start = start_sector as usize;
    let count = sector_count as usize;

    let end = match start.checked_add(count) {
        Some(end) => end,
        None => return false,
    };

    if start < DATA_START_SECTOR || end > TOTAL_SECTORS {
        return false;
    }

    let capacity = match count.checked_mul(SECTOR_SIZE) {
        Some(capacity) => capacity,
        None => return false,
    };

    if data.len() > capacity {
        return false;
    }

    for s in 0..count {
        let offset = s * SECTOR_SIZE;
        let mut sector = [0u8; SECTOR_SIZE];

        if offset < data.len() {
            let copy_end = (offset + SECTOR_SIZE).min(data.len());
            sector[..copy_end - offset].copy_from_slice(&data[offset..copy_end]);
        }

        if !write_sector(start + s, &sector) {
            return false;
        }
    }

    true
}

/// Format the disk with a fresh AIVFS superblock and empty inode table.
///
/// WARNING: This is destructive. It overwrites filesystem metadata.
pub fn format() -> bool {
    let sb = Superblock {
        magic: AIVFS_MAGIC,
        version: AIVFS_VERSION,
        total_sectors: TOTAL_SECTORS as u32,
        inode_table_start: INODE_TABLE_START as u32,
        inode_table_sectors: INODE_TABLE_SECTORS as u32,
        data_start_sector: DATA_START_SECTOR as u32,
        label: [0u8; 32],
    };

    if !write_superblock(&sb) {
        return false;
    }

    let blank_inodes: Vec<Inode> = (0..MAX_INODES)
        .map(|_| Inode {
            name: [0u8; 32],
            size: 0,
            created_at: 0,
            data_start_sector: 0,
            sector_count: 0,
            flags: FLAG_UNUSED,
            semantic_tag_hash: 0,
            embedding_sector: 0,
            _reserved: [0u8; 5],
        })
        .collect();

    write_inode_table(&blank_inodes)
}

/// Return a file's starting sector, allocated sector count, and byte size.
pub fn get_file_info(name: &str) -> Option<(u32, u16, u32)> {
    let inodes = read_inode_table()?;

    for node in &inodes {
        if node.flags != FLAG_FILE {
            continue;
        }

        // Skip malformed entries instead of failing the whole search.
        let stored_name = match inode_name(node) {
            Some(stored_name) => stored_name,
            None => continue,
        };

        if stored_name == name {
            return Some((node.data_start_sector, node.sector_count, node.size));
        }
    }

    None
}

/// Validate the filesystem superblock and inode table without formatting.
pub fn mount() -> bool {
    let sb = match read_superblock() {
        Some(sb) => sb,
        None => return false,
    };

    if sb.magic != AIVFS_MAGIC
        || sb.version != AIVFS_VERSION
        || sb.total_sectors != TOTAL_SECTORS as u32
        || sb.inode_table_start != INODE_TABLE_START as u32
        || sb.inode_table_sectors != INODE_TABLE_SECTORS as u32
        || sb.data_start_sector != DATA_START_SECTOR as u32
    {
        return false;
    }

    // Mount if the inode table can be read successfully.
    read_inode_table().is_some()
}

pub fn calculate_semantic_tag_hash(tag: &str) -> u32 {
    let mut hash: u32 = 2166136261;

    for byte in tag.bytes() {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(16777619);
    }

    hash
}

pub fn write_file_with_tag(name: &str, data: &[u8], tag: &str) -> bool {
    let hash = calculate_semantic_tag_hash(tag);
    write_file_internal(name, data, hash)
}

pub fn write_file(name: &str, data: &[u8]) -> bool {
    let hash = calculate_semantic_tag_hash(name);
    write_file_internal(name, data, hash)
}

fn write_file_internal(name: &str, data: &[u8], tag_hash: u32) -> bool {
    let name_bytes = name.as_bytes();

    if name_bytes.is_empty()
        || name_bytes.len() > 31
        || name_bytes.contains(&0)
        || data.len() > (TOTAL_SECTORS - DATA_START_SECTOR) * SECTOR_SIZE
        || data.len() > u32::MAX as usize
    {
        return false;
    }

    let mut inodes = match read_inode_table() {
        Some(inodes) => inodes,
        None => return false,
    };

    // Validate the current allocation map before making changes.
    if find_free_data_sector(&inodes).is_none() {
        return false;
    }

    let needed_sectors = sectors_needed(data.len());

    let existing_slot = match inodes
        .iter()
        .position(|node| node.flags == FLAG_FILE && inode_name(node) == Some(name))
    {
        Some(index) => Some(index),
        None => {
            // Do not create a duplicate if an existing inode has an invalid name.
            if inodes
                .iter()
                .any(|node| node.flags == FLAG_FILE && inode_name(node).is_none())
            {
                return false;
            }
            None
        }
    };

    let slot = match existing_slot {
        Some(index) => index,
        None => match inodes.iter().position(|node| node.flags == FLAG_UNUSED) {
            Some(index) => index,
            None => return false,
        },
    };

    // Allocate new space rather than overwriting data still referenced by an inode.
    let data_sector = match find_free_data_sector(&inodes) {
        Some(sector) => sector,
        None => return false,
    };

    let end_sector = match data_sector.checked_add(u32::from(needed_sectors)) {
        Some(end) => end,
        None => return false,
    };

    if data_sector < DATA_START_SECTOR as u32 || end_sector > TOTAL_SECTORS as u32 {
        return false;
    }

    if !write_data_sectors(data_sector, needed_sectors, data) {
        return false;
    }

    let mut name_buf = [0u8; 32];
    name_buf[..name_bytes.len()].copy_from_slice(name_bytes);

    let created_at = match existing_slot {
        Some(index) => inodes[index].created_at,
        None => 0,
    };

    inodes[slot] = Inode {
        name: name_buf,
        size: data.len() as u32,
        created_at,
        data_start_sector: data_sector,
        sector_count: needed_sectors,
        flags: FLAG_FILE,
        semantic_tag_hash: tag_hash,
        embedding_sector: 0,
        _reserved: [0u8; 5],
    };

    // If this write fails, the newly written data may remain orphaned.
    write_inode_table(&inodes)
}

pub fn find_file_by_semantic_hash(hash: u32) -> Option<Vec<u8>> {
    let inodes = read_inode_table()?;

    for node in &inodes {
        if node.flags != FLAG_FILE || node.semantic_tag_hash != hash {
            continue;
        }

        // Refuse to read files whose stored names are malformed.
        inode_name(node)?;

        let size = node.size as usize;
        let sector_count = node.sector_count as usize;
        let start_sector = node.data_start_sector as usize;

        let capacity = sector_count.checked_mul(SECTOR_SIZE)?;
        let end_sector = start_sector.checked_add(sector_count)?;

        if size > capacity || start_sector < DATA_START_SECTOR || end_sector > TOTAL_SECTORS {
            return None;
        }

        let mut data = Vec::with_capacity(size);

        for sector_index in 0..sector_count {
            let mut sector = [0u8; SECTOR_SIZE];

            if !read_sector(start_sector + sector_index, &mut sector) {
                return None;
            }

            data.extend_from_slice(&sector);
        }

        data.truncate(size);
        return Some(data);
    }

    None
}

pub fn find_filename_by_semantic_hash(hash: u32) -> Option<String> {
    let inodes = read_inode_table()?;

    for node in &inodes {
        if node.flags == FLAG_FILE && node.semantic_tag_hash == hash {
            return Some(String::from(inode_name(node)?));
        }
    }

    None
}

pub fn read_file(name: &str) -> Option<Vec<u8>> {
    let inodes = read_inode_table()?;

    for node in &inodes {
        if node.flags != FLAG_FILE {
            continue;
        }

        // Skip malformed entries without blocking other valid files.
        let stored_name = match inode_name(node) {
            Some(stored_name) => stored_name,
            None => continue,
        };

        if stored_name != name {
            continue;
        }

        let size = node.size as usize;
        let sector_count = node.sector_count as usize;
        let start_sector = node.data_start_sector as usize;

        let capacity = sector_count.checked_mul(SECTOR_SIZE)?;
        let end_sector = start_sector.checked_add(sector_count)?;

        if size > capacity || start_sector < DATA_START_SECTOR || end_sector > TOTAL_SECTORS {
            return None;
        }

        let mut data = Vec::with_capacity(size);

        for sector_index in 0..sector_count {
            let mut sector = [0u8; SECTOR_SIZE];

            if !read_sector(start_sector + sector_index, &mut sector) {
                return None;
            }

            data.extend_from_slice(&sector);
        }

        data.truncate(size);
        return Some(data);
    }

    None
}

pub fn delete_file(name: &str) -> bool {
    let mut inodes = match read_inode_table() {
        Some(inodes) => inodes,
        None => return false,
    };

    for node in inodes.iter_mut() {
        if node.flags != FLAG_FILE {
            continue;
        }

        if inode_name(node) == Some(name) {
            *node = Inode {
                name: [0u8; 32],
                size: 0,
                created_at: 0,
                data_start_sector: 0,
                sector_count: 0,
                flags: FLAG_UNUSED,
                semantic_tag_hash: 0,
                embedding_sector: 0,
                _reserved: [0u8; 5],
            };

            // The file's old sectors are not erased or reclaimed by this format.
            return write_inode_table(&inodes);
        }
    }

    false
}

pub fn list_files() -> Vec<String> {
    let mut names = Vec::new();

    if let Some(inodes) = read_inode_table() {
        for node in &inodes {
            if node.flags != FLAG_FILE {
                continue;
            }

            if let Some(name) = inode_name(node) {
                names.push(String::from(name));
            }
        }
    }

    names
}

pub fn list_files_with_info() -> Vec<(String, u32)> {
    let mut files = Vec::new();

    if let Some(inodes) = read_inode_table() {
        for node in &inodes {
            if node.flags != FLAG_FILE {
                continue;
            }

            if let Some(name) = inode_name(node) {
                files.push((String::from(name), node.size));
            }
        }
    }

    files.sort_by(|left, right| left.0.cmp(&right.0));
    files
}

/// Destructive smoke test. Do not call during normal boot.
pub fn run_smoke_test() -> bool {
    if !format() {
        return false;
    }

    if !mount() {
        return false;
    }

    if !write_file("hello.txt", b"Hello from AIVFS!") {
        return false;
    }

    let contents = match read_file("hello.txt") {
        Some(data) => data,
        None => return false,
    };

    if contents != b"Hello from AIVFS!" {
        return false;
    }

    let files = list_files();

    if !files.iter().any(|name| name == "hello.txt") {
        return false;
    }

    delete_file("hello.txt")
}
