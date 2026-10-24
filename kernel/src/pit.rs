use x86_64::instructions::port::Port;

pub fn init_pit() {
    pub const FREQ_HZ: u64 = 1000;
    let divisor: u64 = 1_193_182 / FREQ_HZ;
    let mut command_register = Port::<u8>::new(0x43);
    let mut data_register_timer = Port::<u8>::new(0x40);

    unsafe {
        command_register.write(0x36);
        data_register_timer.write((divisor & 0xFF) as u8);
        data_register_timer.write(((divisor >> 8) & 0xFF) as u8);
    }
}
