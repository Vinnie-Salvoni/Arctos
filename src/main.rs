use std::env;
use std::path::PathBuf;
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let mode = env::args()
        .nth(1)
        .unwrap_or_else(|| "bios".to_string())
        .to_lowercase();

    if mode != "bios" {
        eprintln!("Usage: cargo run -- [bios]");
        eprintln!("This launcher currently supports BIOS boot.");
        return ExitCode::from(2);
    }

    let bios_image = PathBuf::from(env!("BIOS_PATH"));

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let test_disk = manifest_dir.join("target").join("test_disk.bin");

    if !bios_image.exists() {
        eprintln!("Bootable image not found: {}", bios_image.display());
        eprintln!("Try: cargo build");
        return ExitCode::from(1);
    }

    if !test_disk.exists() {
        eprintln!("Arctos test disk not found: {}", test_disk.display());
        eprintln!("Try: cargo build");
        return ExitCode::from(1);
    }

    println!("Starting Arctos OS in QEMU (BIOS mode)...");
    println!("Boot image: {}", bios_image.display());
    println!("Data disk:  {}", test_disk.display());

    let status = Command::new("qemu-system-x86_64")
        .arg("-m")
        .arg("4096M")
        .arg("-drive")
        .arg(format!(
            "file={},format=raw,index=0,media=disk",
            bios_image.display()
        ))
        .arg("-drive")
        .arg(format!(
            "file={},format=raw,index=1,media=disk",
            test_disk.display()
        ))
        .arg("-netdev")
        .arg("user,id=net0")
        .arg("-device")
        .arg("e1000,netdev=net0")
        .arg("-serial")
        .arg("tcp:127.0.0.1:4444,server=on,wait=off,nodelay=on")
        .arg("-rtc")
        .arg("clock=host")
        .status();

    match status {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(status) => {
            eprintln!("QEMU exited with status: {status}");
            ExitCode::from(1)
        }
        Err(error) => {
            eprintln!(
                "Could not launch qemu-system-x86_64: {error}\n\
                 Install QEMU and make sure qemu-system-x86_64 is on PATH."
            );
            ExitCode::from(1)
        }
    }
}
