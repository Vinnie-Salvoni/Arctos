use spin::Mutex;

const MAX_EVENTS: usize = 64;

struct BootLog {
    events: [&'static str; MAX_EVENTS],
    len: usize,
}

static EVENTS: Mutex<BootLog> = Mutex::new(BootLog {
    events: [""; MAX_EVENTS],
    len: 0,
});

/// Record an event without allocating heap memory.
pub fn record(message: &'static str) {
    let mut log = EVENTS.lock();

    if log.len < MAX_EVENTS {
        let index = log.len;
        log.events[index] = message;
        log.len += 1;
    }
}

/// Print the recorded startup events.
pub fn print() {
    let log = EVENTS.lock();

    crate::println!("--- Arctos Boot Log ---");

    if log.len == 0 {
        crate::println!("No boot events recorded.");
        return;
    }

    for index in 0..log.len {
        crate::println!("{:02}. {}", index + 1, log.events[index]);
    }

    crate::println!("--- End of Boot Log ---");
}
