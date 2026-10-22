use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;

/// A small persistent record of kernel startup events.
static EVENTS: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Record a startup event for later inspection.
pub fn record(message: &str) {
    let mut events = EVENTS.lock();

    // Keep the log bounded so it cannot grow indefinitely.
    if events.len() >= 64 {
        events.remove(0);
    }

    events.push(String::from(message));
}

/// Print the recorded startup events.
pub fn print() {
    let events = EVENTS.lock();

    crate::println!("--- Arctos Boot Log ---");

    if events.is_empty() {
        crate::println!("No boot events recorded.");
        return;
    }

    for (index, event) in events.iter().enumerate() {
        crate::println!("{:02}. {}", index + 1, event);
    }

    crate::println!("--- End of Boot Log ---");
}
