use std::convert::TryInto;

const SECTOR_SIZE: usize = 512;
const FLAG_UNUSED: u8 = 0;
const FLAG_FILE: u8 = 1;

struct ParsedInode {
    name: String,
    size: u32,
    created_at: u64,
    data_start_sector: u32,
    sector_count: u16,
    flags: u8,
}

fn parse_inode(bytes: &[u8]) -> ParsedInode {
    let mut name_bytes = [0u8; 16];
    name_bytes.copy_from_slice(&bytes[0..16]);
    let name = String::from_utf8_lossy(&name_bytes).trim_matches('\0').to_string();
    
    let size = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
    let created_at = u64::from_le_bytes(bytes[20..28].try_into().unwrap());
    let data_start_sector = u32::from_le_bytes(bytes[28..32].try_into().unwrap());
    let sector_count = u16::from_le_bytes(bytes[32..34].try_into().unwrap());
    let flags = bytes[34];
    
    ParsedInode {
        name,
        size,
        created_at,
        data_start_sector,
        sector_count,
        flags,
    }
}

fn serialize_inode(inode: &ParsedInode) -> [u8; 64] {
    let mut bytes = [0u8; 64];
    let name_bytes = inode.name.as_bytes();
    let copy_len = name_bytes.len().min(15);
    bytes[0..copy_len].copy_from_slice(&name_bytes[..copy_len]);
    
    bytes[16..20].copy_from_slice(&inode.size.to_le_bytes());
    bytes[20..28].copy_from_slice(&inode.created_at.to_le_bytes());
    bytes[28..32].copy_from_slice(&inode.data_start_sector.to_le_bytes());
    bytes[32..34].copy_from_slice(&inode.sector_count.to_le_bytes());
    bytes[34] = inode.flags;
    bytes
}

fn build_app(elf_path: &str, output_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let elf_bytes = std::fs::read(elf_path)?;
    if elf_bytes.len() < 64 {
        return Err("ELF file too small".into());
    }
    if &elf_bytes[0..4] != b"\x7fELF" {
        return Err("Not a valid ELF file".into());
    }
    
    let entry_point = u64::from_le_bytes(elf_bytes[24..32].try_into()?);
    let phoff = u64::from_le_bytes(elf_bytes[32..40].try_into()?) as usize;
    let phentsize = u16::from_le_bytes(elf_bytes[54..56].try_into()?) as usize;
    let phnum = u16::from_le_bytes(elf_bytes[56..58].try_into()?) as usize;
    
    let mut app_payload = Vec::new();
    let base_addr = 0x4000_0000_0000;
    
    for i in 0..phnum {
        let offset = phoff + i * phentsize;
        let p_type = u32::from_le_bytes(elf_bytes[offset..offset+4].try_into()?);
        if p_type == 1 { // PT_LOAD
            let p_offset = u64::from_le_bytes(elf_bytes[offset+8..offset+16].try_into()?) as usize;
            let p_vaddr = u64::from_le_bytes(elf_bytes[offset+16..offset+24].try_into()?);
            let p_filesz = u64::from_le_bytes(elf_bytes[offset+32..offset+40].try_into()?) as usize;
            let p_memsz = u64::from_le_bytes(elf_bytes[offset+40..offset+48].try_into()?) as usize;
            
            if p_vaddr >= base_addr && p_vaddr < base_addr + 0x1000000 {
                let segment_offset_in_payload = (p_vaddr - base_addr) as usize;
                
                if app_payload.len() < segment_offset_in_payload + p_memsz {
                    app_payload.resize(segment_offset_in_payload + p_memsz, 0);
                }
                
                if p_filesz > 0 {
                    app_payload[segment_offset_in_payload..segment_offset_in_payload + p_filesz]
                        .copy_from_slice(&elf_bytes[p_offset..p_offset + p_filesz]);
                }
            }
        }
    }
    
    if app_payload.is_empty() {
        return Err("No PT_LOAD segment found in target virtual address range".into());
    }
    
    let entry_offset = (entry_point - base_addr) as u32;
    
    let mut output_bytes = Vec::new();
    output_bytes.extend_from_slice(b"ARCS");
    output_bytes.extend_from_slice(&entry_offset.to_le_bytes());
    output_bytes.extend_from_slice(&(app_payload.len() as u32).to_le_bytes());
    output_bytes.extend_from_slice(&app_payload);
    
    std::fs::write(output_path, output_bytes)?;
    println!("[ok] Successfully built {} (entry offset: {})", output_path, entry_offset);
    Ok(())
}

fn format_disk(disk_data: &mut [u8]) {
    for b in disk_data[..8 * SECTOR_SIZE].iter_mut() {
        *b = 0;
    }
    let magic: u32 = 0xA8C705;
    let version: u16 = 1;
    let total_sectors: u32 = (disk_data.len() / SECTOR_SIZE) as u32;
    let inode_table_start: u32 = 1;
    let inode_table_sectors: u32 = 7;
    let data_start_sector: u32 = 8;
    
    disk_data[0..4].copy_from_slice(&magic.to_le_bytes());
    disk_data[4..6].copy_from_slice(&version.to_le_bytes());
    disk_data[6..10].copy_from_slice(&total_sectors.to_le_bytes());
    disk_data[10..14].copy_from_slice(&inode_table_start.to_le_bytes());
    disk_data[14..18].copy_from_slice(&inode_table_sectors.to_le_bytes());
    disk_data[18..22].copy_from_slice(&data_start_sector.to_le_bytes());
    
    let label = b"ARCTOS-FS";
    disk_data[22..22 + label.len()].copy_from_slice(label);
}

fn write_file_to_disk(disk_path: &str, host_file_path: &str, target_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let file_data = std::fs::read(host_file_path)?;
    let mut disk_data = std::fs::read(disk_path)?;
    
    if disk_data.len() < SECTOR_SIZE {
        return Err("Disk image is too small".into());
    }
    
    let mut magic = u32::from_le_bytes(disk_data[0..4].try_into()?);
    if magic != 0xA8C705 {
        println!("[system] Formatting empty or uninitialized disk image...");
        format_disk(&mut disk_data);
    }
    
    let inode_table_start = u32::from_le_bytes(disk_data[10..14].try_into()?) as usize;
    let inode_table_sectors = u32::from_le_bytes(disk_data[14..18].try_into()?) as usize;
    let data_start_sector = u32::from_le_bytes(disk_data[18..22].try_into()?) as usize;
    
    let max_inodes = inode_table_sectors * (SECTOR_SIZE / 64);
    
    let mut inodes = Vec::new();
    for i in 0..max_inodes {
        let offset = inode_table_start * SECTOR_SIZE + i * 64;
        inodes.push(parse_inode(&disk_data[offset..offset + 64]));
    }
    
    for inode in inodes.iter_mut() {
        if inode.flags == FLAG_FILE && inode.name == target_name {
            inode.flags = FLAG_UNUSED;
            inode.name.clear();
            inode.size = 0;
            inode.sector_count = 0;
            inode.data_start_sector = 0;
        }
    }
    
    let free_slot_idx = inodes.iter().position(|n| n.flags == FLAG_UNUSED)
        .ok_or("No free inode slots available")?;
        
    let mut next_free_sector = data_start_sector;
    for inode in &inodes {
        if inode.flags == FLAG_FILE {
            let end = inode.data_start_sector as usize + inode.sector_count as usize;
            if end > next_free_sector {
                next_free_sector = end;
            }
        }
    }
    
    let needed_sectors = (file_data.len() + SECTOR_SIZE - 1) / SECTOR_SIZE;
    
    let total_needed_size = (next_free_sector + needed_sectors) * SECTOR_SIZE;
    if disk_data.len() < total_needed_size {
        disk_data.resize(total_needed_size, 0);
    }
    
    for s in 0..needed_sectors {
        let file_offset = s * SECTOR_SIZE;
        let disk_offset = (next_free_sector + s) * SECTOR_SIZE;
        let copy_len = (file_data.len() - file_offset).min(SECTOR_SIZE);
        disk_data[disk_offset..disk_offset + copy_len]
            .copy_from_slice(&file_data[file_offset..file_offset + copy_len]);
        if copy_len < SECTOR_SIZE {
            for b in &mut disk_data[disk_offset + copy_len .. disk_offset + SECTOR_SIZE] {
                *b = 0;
            }
        }
    }
    
    inodes[free_slot_idx] = ParsedInode {
        name: target_name.to_string(),
        size: file_data.len() as u32,
        created_at: 0,
        data_start_sector: next_free_sector as u32,
        sector_count: needed_sectors as u16,
        flags: FLAG_FILE,
    };
    
    for i in 0..max_inodes {
        let offset = inode_table_start * SECTOR_SIZE + i * 64;
        let serialized = serialize_inode(&inodes[i]);
        disk_data[offset..offset + 64].copy_from_slice(&serialized);
    }
    
    std::fs::write(disk_path, disk_data)?;
    println!("[ok] Successfully wrote file '{}' to disk image '{}'", target_name, disk_path);
    Ok(())
}

fn delete_file_from_disk(disk_path: &str, target_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut disk_data = std::fs::read(disk_path)?;
    if disk_data.len() < SECTOR_SIZE {
        return Err("Disk image is too small".into());
    }
    
    let magic = u32::from_le_bytes(disk_data[0..4].try_into()?);
    if magic != 0xA8C705 {
        return Err("Not a valid AIVFS disk image".into());
    }
    
    let inode_table_start = u32::from_le_bytes(disk_data[10..14].try_into()?) as usize;
    let inode_table_sectors = u32::from_le_bytes(disk_data[14..18].try_into()?) as usize;
    let max_inodes = inode_table_sectors * (SECTOR_SIZE / 64);
    
    let mut inodes = Vec::new();
    for i in 0..max_inodes {
        let offset = inode_table_start * SECTOR_SIZE + i * 64;
        inodes.push(parse_inode(&disk_data[offset..offset + 64]));
    }
    
    let mut found = false;
    for inode in inodes.iter_mut() {
        if inode.flags == FLAG_FILE && inode.name == target_name {
            inode.flags = FLAG_UNUSED;
            inode.name.clear();
            inode.size = 0;
            inode.sector_count = 0;
            inode.data_start_sector = 0;
            found = true;
        }
    }
    
    if !found {
        return Err(format!("File '{}' not found on disk image", target_name).into());
    }
    
    for i in 0..max_inodes {
        let offset = inode_table_start * SECTOR_SIZE + i * 64;
        let serialized = serialize_inode(&inodes[i]);
        disk_data[offset..offset + 64].copy_from_slice(&serialized);
    }
    
    std::fs::write(disk_path, disk_data)?;
    println!("[ok] Successfully deleted file '{}' from disk image '{}'", target_name, disk_path);
    Ok(())
}

fn list_disk_files(disk_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let disk_data = std::fs::read(disk_path)?;
    if disk_data.len() < SECTOR_SIZE {
        return Err("Disk image too small".into());
    }
    let magic = u32::from_le_bytes(disk_data[0..4].try_into()?);
    if magic != 0xA8C705 {
        return Err("Not a valid AIVFS disk image".into());
    }
    let inode_table_start = u32::from_le_bytes(disk_data[10..14].try_into()?) as usize;
    let inode_table_sectors = u32::from_le_bytes(disk_data[14..18].try_into()?) as usize;
    let max_inodes = inode_table_sectors * (SECTOR_SIZE / 64);
    
    println!("--- Files on Disk Image '{}' ---", disk_path);
    for i in 0..max_inodes {
        let offset = inode_table_start * SECTOR_SIZE + i * 64;
        let inode = parse_inode(&disk_data[offset..offset + 64]);
        if inode.flags == FLAG_FILE {
            println!("- {} ({} bytes, starting sector: {})", inode.name, inode.size, inode.data_start_sector);
        }
    }
    Ok(())
}

const ARCTOS_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/..");

fn compile_and_install(source_path: &str, target_filename: &str) -> Result<(), Box<dyn std::error::Error>> {
    let target_json = format!("{}/x86_64-arctos.json", ARCTOS_ROOT);
    let linker_ld = format!("{}/sdk/app.ld", ARCTOS_ROOT);
    let sdk_rlib = format!("{}/target/x86_64-arctos/debug/libarctos_sdk.rlib", ARCTOS_ROOT);
    let disk_bin = format!("{}/target/test_disk.bin", ARCTOS_ROOT);

    let deps_dir = format!("{}/target/x86_64-arctos/debug/deps", ARCTOS_ROOT);

    println!("[system] Compiling {}...", source_path);
    let output = std::process::Command::new("rustc")
        .args(&[
            "--target", &target_json,
            "-Zunstable-options",
            "--edition", "2021",
            "-L", &deps_dir,
            "-C", &format!("link-arg=-T{}", linker_ld),
            "--extern", &format!("arctos_sdk={}", sdk_rlib),
            source_path,
            "-o", "temp_app.elf"
        ])
        .output()?;
        
    if !output.status.success() {
        return Err(format!("Compilation failed:\n{}", String::from_utf8_lossy(&output.stderr)).into());
    }
    
    println!("[system] Packaging ELF to .arc...");
    build_app("temp_app.elf", "temp_app.arc")?;
    
    println!("[system] Ingesting into test_disk.bin...");
    write_file_to_disk(&disk_bin, "temp_app.arc", target_filename)?;
    
    let _ = std::fs::remove_file("temp_app.elf");
    let _ = std::fs::remove_file("temp_app.arc");
    
    println!("[ok] Build and installation complete!");
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        print_usage();
        return;
    }
    
    let command = &args[1];
    match command.as_str() {
        "compile-and-install" => {
            if args.len() < 4 {
                println!("Usage: cargo run -p arctos_builder -- compile-and-install <source_file> <target_filename>");
                return;
            }
            if let Err(e) = compile_and_install(&args[2], &args[3]) {
                println!("[error] Compile and install failed: {}", e);
            }
        }
        "build-app" => {
            if args.len() < 4 {
                println!("Usage: cargo run -p arctos_builder -- build-app <elf_path> <output_arc_path>");
                return;
            }
            if let Err(e) = build_app(&args[2], &args[3]) {
                println!("[error] Build failed: {}", e);
            }
        }
        "write-disk" => {
            if args.len() < 5 {
                println!("Usage: cargo run -p arctos_builder -- write-disk <disk_path> <host_file_path> <target_filename>");
                return;
            }
            if let Err(e) = write_file_to_disk(&args[2], &args[3], &args[4]) {
                println!("[error] Write disk failed: {}", e);
            }
        }
        "delete-file" => {
            if args.len() < 4 {
                println!("Usage: cargo run -p arctos_builder -- delete-file <disk_path> <target_filename>");
                return;
            }
            if let Err(e) = delete_file_from_disk(&args[2], &args[3]) {
                println!("[error] Delete file failed: {}", e);
            }
        }
        "list-disk" => {
            if args.len() < 3 {
                println!("Usage: cargo run -p arctos_builder -- list-disk <disk_path>");
                return;
            }
            if let Err(e) = list_disk_files(&args[2]) {
                println!("[error] List disk failed: {}", e);
            }
        }
        _ => {
            print_usage();
        }
    }
}

fn print_usage() {
    println!("Arctos OS Host Build Utility Commands:");
    println!("  compile-and-install <source_file> <name> - Automatically compiles, packages, and injects app");
    println!("  build-app <elf_path> <output_arc_path>   - Compiles compiled ELF to PIO raw .arc");
    println!("  write-disk <disk_path> <file> <name>    - Injects a host file into the raw AIVFS disk image");
    println!("  delete-file <disk_path> <name>          - Deletes a file from the raw AIVFS disk image");
    println!("  list-disk <disk_path>                    - Lists all files contained inside raw AIVFS disk");
}
