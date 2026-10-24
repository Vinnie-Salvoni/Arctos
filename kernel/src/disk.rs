use x86_64::instructions::port::Port;

const DATA_PORT: u16 = 0x1F0;
const SECTOR_COUNT_PORT: u16 = 0x1F2;
const LBA_LO_PORT: u16 = 0x1F3;
const LBA_MID_PORT: u16 = 0x1F4;
const LBA_HI_PORT: u16 = 0x1F5;
const DRIVE_HEAD_PORT: u16 = 0x1F6;
const STATUS_COMMAND_PORT: u16 = 0x1F7;
const ALT_STATUS_PORT: u16 = 0x3F6; // read = alternate status, write = device control

const STATUS_BSY: u8 = 0x80;
const STATUS_DF: u8 = 0x20; // device fault
const STATUS_DRQ: u8 = 0x08;
const STATUS_ERR: u8 = 0x01;

#[allow(dead_code)]
const DRIVE_MASTER: u8 = 0xE0;
const DRIVE_SLAVE: u8 = 0xF0;
const ACTIVE_DRIVE: u8 = DRIVE_SLAVE;

const POLL_LIMIT: u32 = 1_000_000;

/// ATA needs ~400 ns after a drive select; four alt-status reads is the standard delay.
fn io_delay() {
    let mut alt = Port::<u8>::new(ALT_STATUS_PORT);
    for _ in 0..4 {
        unsafe {
            alt.read();
        }
    }
}

/// Waits until the drive is not busy. Err on error, fault or timeout.
fn wait_not_busy() -> Result<(), ()> {
    let mut status_port = Port::<u8>::new(STATUS_COMMAND_PORT);
    for _ in 0..POLL_LIMIT {
        let status = unsafe { status_port.read() };
        if status & STATUS_BSY == 0 {
            if status & (STATUS_ERR | STATUS_DF) != 0 {
                return Err(());
            }
            return Ok(());
        }
    }
    Err(())
}

/// Waits until the drive is ready to transfer a sector (DRQ). Err on error, fault or timeout.
fn wait_ready() -> Result<(), ()> {
    let mut status_port = Port::<u8>::new(STATUS_COMMAND_PORT);
    for _ in 0..POLL_LIMIT {
        let status = unsafe { status_port.read() };
        if status & STATUS_BSY == 0 {
            if status & (STATUS_ERR | STATUS_DF) != 0 {
                return Err(());
            }
            if status & STATUS_DRQ != 0 {
                return Ok(());
            }
        }
    }
    Err(())
}

/// Selects the drive, programs the LBA registers and sends `command`.
fn issue(lba_addr: u32, sectors: u8, command: u8) -> Result<(), ()> {
    let mut sector_count_port = Port::<u8>::new(SECTOR_COUNT_PORT);
    let mut lba_lo = Port::<u8>::new(LBA_LO_PORT);
    let mut lba_mid = Port::<u8>::new(LBA_MID_PORT);
    let mut lba_hi = Port::<u8>::new(LBA_HI_PORT);
    let mut drive_head = Port::<u8>::new(DRIVE_HEAD_PORT);
    let mut command_port = Port::<u8>::new(STATUS_COMMAND_PORT);

    unsafe {
        Port::<u8>::new(ALT_STATUS_PORT).write(0x02); // nIEN: we poll, so mask ATA IRQs
        drive_head.write(ACTIVE_DRIVE | (((lba_addr >> 24) & 0x0F) as u8));
    }
    io_delay();
    wait_not_busy()?;

    unsafe {
        sector_count_port.write(sectors);
        lba_lo.write((lba_addr & 0xFF) as u8);
        lba_mid.write(((lba_addr >> 8) & 0xFF) as u8);
        lba_hi.write(((lba_addr >> 16) & 0xFF) as u8);
        command_port.write(command);
    }
    Ok(())
}

/// Reads `sectors` sectors starting at `lba_addr` into `buffer` (256 u16 words per sector).
pub fn read_sectors(lba_addr: u32, sectors: u8, buffer: &mut [u16]) -> Result<(), ()> {
    if sectors == 0 || buffer.len() < (sectors as usize) * 256 {
        return Err(());
    }
    issue(lba_addr, sectors, 0x20)?; // READ SECTORS

    let mut data_port = Port::<u16>::new(DATA_PORT);
    for sector in 0..(sectors as usize) {
        wait_ready()?;
        for word in 0..256 {
            buffer[sector * 256 + word] = unsafe { data_port.read() };
        }
    }
    Ok(())
}

/// Writes `sectors` sectors from `buffer` to `lba_addr` and flushes the drive cache.
pub fn write_sectors(lba_addr: u32, sectors: u8, buffer: &[u16]) -> Result<(), ()> {
    if sectors == 0 || buffer.len() < (sectors as usize) * 256 {
        return Err(());
    }
    issue(lba_addr, sectors, 0x30)?; // WRITE SECTORS

    let mut data_port = Port::<u16>::new(DATA_PORT);
    for sector in 0..(sectors as usize) {
        wait_ready()?;
        for word in 0..256 {
            unsafe { data_port.write(buffer[sector * 256 + word]) };
        }
    }

    // The drive goes busy while it commits the last sector, then FLUSH CACHE.
    // A flush never raises DRQ, so wait for "not busy", not for DRQ.
    wait_not_busy()?;
    unsafe {
        Port::<u8>::new(STATUS_COMMAND_PORT).write(0xE7);
    }
    wait_not_busy()
}
