pub mod structures;

use alloc::string::String;
use alloc::vec::Vec;
// spin::Mutex removed
use structures::{Inode, Superblock};

const AIVFS_MAGIC: u32 = 0xA8C705;
const AIVFS_VERSION: u16 = 1;
const SECTOR_SIZE: usize = 512;
const TOTAL_SECTORS: usize = 20480; // 10 MB drive capacity
const SUPERBLOCK_SECTOR: usize = 0;
const INODE_TABLE_START: usize = 1;
const INODE_TABLE_SECTORS: usize = 7;
const INODES_PER_SECTOR: usize = 8;
const MAX_INODES: usize = INODE_TABLE_SECTORS * INODES_PER_SECTOR;
const DATA_START_SECTOR: usize = 8;
const FLAG_UNUSED: u8 = 0;
const FLAG_FILE: u8 = 1;

fn sectors_needed(bytes: usize) -> u16 {
    ((bytes + SECTOR_SIZE - 1) / SECTOR_SIZE) as u16
}

/// Reads a sector from the physical disk using the ATA PIO driver.
fn read_sector(sector: usize, out: &mut [u8; SECTOR_SIZE]) {
    let mut temp = [0u16; 256];
    if crate::disk::read_sectors(sector as u32, 1, &mut temp).is_ok() {
        for i in 0..256 {
            let bytes = temp[i].to_le_bytes();
            out[i * 2] = bytes[0];
            out[i * 2 + 1] = bytes[1];
        }
    }
}

/// Writes a sector to the physical disk using the ATA PIO driver.
fn write_sector(sector: usize, bytes: &[u8; SECTOR_SIZE]) {
    let mut temp = [0u16; 256];
    for i in 0..256 {
        temp[i] = u16::from_le_bytes([bytes[i * 2], bytes[i * 2 + 1]]);
    }
    let _ = crate::disk::write_sectors(sector as u32, 1, &temp);
}

fn read_superblock() -> Option<Superblock> {
    let mut sector = [0u8; SECTOR_SIZE];
    read_sector(SUPERBLOCK_SECTOR, &mut sector);

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
    write_sector(SUPERBLOCK_SECTOR, &sector);
    true
}

pub fn read_inode_table() -> Option<Vec<Inode>> {
    let mut inodes = Vec::new();
    for s in 0..INODE_TABLE_SECTORS {
        let mut sector = [0u8; SECTOR_SIZE];
        read_sector(INODE_TABLE_START + s as usize, &mut sector);

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

fn write_inode_table(inodes: &[Inode]) -> bool {
    for s in 0..INODE_TABLE_SECTORS as usize {
        let mut sector = [0u8; SECTOR_SIZE];
        for i in 0..INODES_PER_SECTOR {
            let idx = s * INODES_PER_SECTOR + i;
            if idx >= inodes.len() {
                break;
            }
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
            sector[off + 59..off + 64].copy_from_slice(&node._reserved[..5]);
        }
        write_sector(INODE_TABLE_START + s, &sector);
    }
    true
}

fn find_free_data_sector(inodes: &[Inode]) -> u32 {
    let mut next = DATA_START_SECTOR as u32;
    for node in inodes {
        if node.flags != FLAG_UNUSED {
            let end = node.data_start_sector + node.sector_count as u32;
            if end > next {
                next = end;
            }
        }
    }
    next
}

fn write_data_sectors(start_sector: u32, sector_count: u16, data: &[u8]) -> bool {
    for s in 0..sector_count as usize {
        let offset = s * SECTOR_SIZE;
        let mut sector = [0u8; SECTOR_SIZE];
        if offset < data.len() {
            let end = (offset + SECTOR_SIZE).min(data.len());
            sector[..end - offset].copy_from_slice(&data[offset..end]);
        }
        write_sector(start_sector as usize + s, &sector);
    }
    true
}

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

pub fn get_file_info(name: &str) -> Option<(u32, u16, u32)> {
    let inodes = read_inode_table()?;
    for node in &inodes {
        if node.flags != FLAG_FILE {
            continue;
        }
        let existing_name = core::str::from_utf8(&node.name).unwrap_or("").trim_matches('\0');
        if existing_name == name {
            return Some((node.data_start_sector, node.sector_count, node.size));
        }
    }
    None
}

pub fn mount() -> bool {
    match read_superblock() {
        Some(sb) => sb.magic == AIVFS_MAGIC,
        None => false,
    }
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
    let mut inodes = match read_inode_table() {
        Some(v) => v,
        None => return false,
    };

    let needed_sectors = sectors_needed(data.len());

    for node in inodes.iter_mut() {
        if node.flags != FLAG_FILE {
            continue;
        }
        let existing_name = core::str::from_utf8(&node.name).unwrap_or("").trim_matches('\0');
        if existing_name == name {
            node.size = data.len() as u32;
            node.sector_count = needed_sectors;
            node.semantic_tag_hash = tag_hash;
            if write_data_sectors(node.data_start_sector, needed_sectors, data) {
                return write_inode_table(&inodes);
            }
            return false;
        }
    }

    let free_slot = inodes.iter().position(|n| n.flags == FLAG_UNUSED);
    let slot = match free_slot {
        Some(s) => s,
        None => return false,
    };

    let data_sector = find_free_data_sector(&inodes);
    let mut name_buf = [0u8; 32];
    let name_bytes = name.as_bytes();
    let copy_len = name_bytes.len().min(31);
    name_buf[..copy_len].copy_from_slice(&name_bytes[..copy_len]);

    inodes[slot] = Inode {
        name: name_buf,
        size: data.len() as u32,
        created_at: 0,
        data_start_sector: data_sector,
        sector_count: needed_sectors,
        flags: FLAG_FILE,
        semantic_tag_hash: tag_hash,
        embedding_sector: 0,
        _reserved: [0u8; 5],
    };

    if !write_data_sectors(data_sector, needed_sectors, data) {
        return false;
    }
    write_inode_table(&inodes)
}

pub fn find_file_by_semantic_hash(hash: u32) -> Option<Vec<u8>> {
    let inodes = read_inode_table()?;
    for node in &inodes {
        if node.flags == FLAG_FILE && node.semantic_tag_hash == hash {
            let mut data = Vec::with_capacity(node.size as usize);
            for s in 0..node.sector_count as usize {
                let mut sector = [0u8; SECTOR_SIZE];
                read_sector(node.data_start_sector as usize + s, &mut sector);
                data.extend_from_slice(&sector);
            }
            data.truncate(node.size as usize);
            return Some(data);
        }
    }
    None
}

pub fn find_filename_by_semantic_hash(hash: u32) -> Option<String> {
    let inodes = read_inode_table()?;
    for node in &inodes {
        if node.flags == FLAG_FILE && node.semantic_tag_hash == hash {
            if let Ok(s) = core::str::from_utf8(&node.name) {
                let trimmed = s.trim_matches('\0');
                if !trimmed.is_empty() {
                    return Some(String::from(trimmed));
                }
            }
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
        let existing_name = core::str::from_utf8(&node.name).unwrap_or("").trim_matches('\0');
        if existing_name != name {
            continue;
        }

        let mut data = Vec::with_capacity(node.size as usize);
        for s in 0..node.sector_count as usize {
            let mut sector = [0u8; SECTOR_SIZE];
            read_sector(node.data_start_sector as usize + s, &mut sector);
            data.extend_from_slice(&sector);
        }
        data.truncate(node.size as usize);
        return Some(data);
    }
    None
}

pub fn delete_file(name: &str) -> bool {
    let mut inodes = match read_inode_table() {
        Some(v) => v,
        None => return false,
    };

    for node in inodes.iter_mut() {
        if node.flags != FLAG_FILE {
            continue;
        }
        let existing_name = core::str::from_utf8(&node.name).unwrap_or("").trim_matches('\0');
        if existing_name == name {
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
            return write_inode_table(&inodes);
        }
    }
    false
}

pub fn list_files() -> Vec<String> {
    let mut names = Vec::new();
    if let Some(inodes) = read_inode_table() {
        for node in &inodes {
            if node.flags == FLAG_FILE {
                if let Ok(s) = core::str::from_utf8(&node.name) {
                    let trimmed = s.trim_matches('\0');
                    if !trimmed.is_empty() {
                        names.push(String::from(trimmed));
                    }
                }
            }
        }
    }
    names
}

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
