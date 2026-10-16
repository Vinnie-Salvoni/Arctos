use x86_64::instructions::port::Port;

// We use 16-bit ports for data transfer, and 8-bit ports for commands/status.
const DATA_PORT: u16 = 0x1F0;
const SECTOR_COUNT_PORT: u16 = 0x1F2;
const LBA_LO_PORT: u16 = 0x1F3;
const LBA_MID_PORT: u16 = 0x1F4;
const LBA_HI_PORT: u16 = 0x1F5;
const DRIVE_HEAD_PORT: u16 = 0x1F6;
const STATUS_COMMAND_PORT: u16 = 0x1F7;

// Status register flags (Port 0x1F7)
const STATUS_BSY: u8 = 0x80; // Busy bit: Controller is executing a command
const STATUS_DRQ: u8 = 0x08; // Data Request bit: Controller is ready to send/receive data
const STATUS_ERR: u8 = 0x01; // Error bit: An error occurred

// ATA Drive Selection Flags:
// 0xE0: LBA mode + Master drive (Drive 0 - Boot image)
// 0xF0: LBA mode + Slave drive (Drive 1 - Secondary data storage)
#[allow(dead_code)]
const DRIVE_MASTER: u8 = 0xE0;
const DRIVE_SLAVE: u8 = 0xF0;
const ACTIVE_DRIVE: u8 = DRIVE_SLAVE;

/// Polls the disk status register until the drive is ready to transfer data.
/// Includes a safety timeout to prevent permanent hangs.
fn wait_ready() -> Result<(), ()> {
    let mut status_port = Port::<u8>::new(STATUS_COMMAND_PORT);
    let mut timeout = 0;
    
    loop {
        let status = unsafe { status_port.read() };
        
        // If the Busy flag (BSY) is clear, check status bits
        if (status & STATUS_BSY) == 0 {
            // Once BSY is clear, check if the Data Request (DRQ) is set.
            if (status & STATUS_DRQ) != 0 {
                return Ok(());
            }
            // If there's an error status, fail immediately.
            if (status & STATUS_ERR) != 0 {
                return Err(());
            }
        }

        timeout += 1;
        if timeout > 100_000 {
            return Err(()); // Timeout failure
        }
    }
}

/// Reads `sectors` number of sectors starting from LBA `lba_addr` into the buffer.
/// The buffer must be sized to hold the requested sectors (512 bytes, i.e., 256 `u16` words per sector).
pub fn read_sectors(lba_addr: u32, sectors: u8, buffer: &mut [u16]) -> Result<(), ()> {
    // We only support reading into a buffer that matches the sector count size
    assert!(buffer.len() >= (sectors as usize) * 256);

    let mut data_port = Port::<u16>::new(DATA_PORT);
    let mut sector_count_port = Port::<u8>::new(SECTOR_COUNT_PORT);
    let mut lba_lo = Port::<u8>::new(LBA_LO_PORT);
    let mut lba_mid = Port::<u8>::new(LBA_MID_PORT);
    let mut lba_hi = Port::<u8>::new(LBA_HI_PORT);
    let mut drive_head = Port::<u8>::new(DRIVE_HEAD_PORT);
    let mut command_port = Port::<u8>::new(STATUS_COMMAND_PORT);

    unsafe {
        // Disable ATA interrupts on the primary controller
        Port::<u8>::new(0x3F6).write(0x02);

        // 1. Send the drive selection and LBA address high bits (24-27)
        drive_head.write(ACTIVE_DRIVE | (((lba_addr >> 24) & 0x0F) as u8));
        
        // 2. Set the sector count to read
        sector_count_port.write(sectors);
        
        // 3. Send the LBA address low, mid, and high bytes
        lba_lo.write((lba_addr & 0xFF) as u8);
        lba_mid.write(((lba_addr >> 8) & 0xFF) as u8);
        lba_hi.write(((lba_addr >> 16) & 0xFF) as u8);
        
        // 4. Send the Command: Read Sectors (0x20)
        command_port.write(0x20);
    }

    // 5. Read the data sector-by-sector
    for sector in 0..(sectors as usize) {
        // Wait for the controller to make the sector data available
        wait_ready()?;
        
        // Read 256 u16 words (512 bytes) from the data register
        for word in 0..256 {
            let index = sector * 256 + word;
            buffer[index] = unsafe { data_port.read() };
        }
    }

    Ok(())
}

/// Writes `sectors` number of sectors starting from LBA `lba_addr` from the buffer to the disk.
pub fn write_sectors(lba_addr: u32, sectors: u8, buffer: &[u16]) -> Result<(), ()> {
    assert!(buffer.len() >= (sectors as usize) * 256);

    let mut data_port = Port::<u16>::new(DATA_PORT);
    let mut sector_count_port = Port::<u8>::new(SECTOR_COUNT_PORT);
    let mut lba_lo = Port::<u8>::new(LBA_LO_PORT);
    let mut lba_mid = Port::<u8>::new(LBA_MID_PORT);
    let mut lba_hi = Port::<u8>::new(LBA_HI_PORT);
    let mut drive_head = Port::<u8>::new(DRIVE_HEAD_PORT);
    let mut command_port = Port::<u8>::new(STATUS_COMMAND_PORT);

    unsafe {
        // Disable ATA interrupts on the primary controller
        Port::<u8>::new(0x3F6).write(0x02);

        // 1. Send the drive selection and LBA address high bits
        drive_head.write(ACTIVE_DRIVE | (((lba_addr >> 24) & 0x0F) as u8));
        
        // 2. Set the sector count to write
        sector_count_port.write(sectors);
        
        // 3. Send the LBA address bytes
        lba_lo.write((lba_addr & 0xFF) as u8);
        lba_mid.write(((lba_addr >> 8) & 0xFF) as u8);
        lba_hi.write(((lba_addr >> 16) & 0xFF) as u8);
        
        // 4. Send the Command: Write Sectors (0x30)
        command_port.write(0x30);
    }

    // 5. Write the data sector-by-sector
    for sector in 0..(sectors as usize) {
        // Wait for the controller to be ready to accept data
        wait_ready()?;
        
        // Write 256 u16 words to the data register
        for word in 0..256 {
            let index = sector * 256 + word;
            unsafe { data_port.write(buffer[index]) };
        }
    }

    // 6. Send the Command: Cache Flush (0xE7) to force disk sync
    unsafe {
        command_port.write(0xE7);
    }
    
    // Wait for the sync command execution to complete
    wait_ready()?;

    Ok(())
}