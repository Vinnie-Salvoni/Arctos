use std::path::PathBuf;
use std::process::Command;

fn main() {
    let mut raw_args = std::env::args().skip(1);
    let mut kernel_path: Option<PathBuf> = None;
    let mut use_bios = false;
    let mut extra_args = Vec::new();

    while let Some(arg) = raw_args.next() {
        if arg == "--bios" {
            use_bios = true;
        } else if kernel_path.is_none() && !arg.starts_with('-') {
            kernel_path = Some(PathBuf::from(arg));
        } else {
            extra_args.push(arg);
        }
    }

    let kernel_binary_path = match kernel_path {
        Some(path) => path,
        None => {
            eprintln!("Usage: arctos_runner <kernel-binary-path> [--bios] [extra qemu args...]");
            std::process::exit(1);
        }
    };

    let target_dir = kernel_binary_path
        .parent()
        .expect("kernel binary has no parent directory");

    let uefi_path = target_dir.join("arctos-uefi.img");
    let bios_path = target_dir.join("arctos-bios.img");

    println!("[runner] Creating UEFI disk image at: {}", uefi_path.display());
    bootloader::UefiBoot::new(&kernel_binary_path)
        .create_disk_image(&uefi_path)
        .expect("Failed to create UEFI disk image");

    println!("[runner] Creating BIOS disk image at: {}", bios_path.display());
    bootloader::BiosBoot::new(&kernel_binary_path)
        .create_disk_image(&bios_path)
        .expect("Failed to create BIOS disk image");

    println!(
        "[runner] Booting QEMU in {} mode...",
        if use_bios { "Legacy BIOS" } else { "1080p UEFI" }
    );

    let mut qemu = Command::new("qemu-system-x86_64");

    if use_bios {
        qemu.arg("-drive")
            .arg(format!("format=raw,file={}", bios_path.display()));
    } else {
        qemu.arg("-bios").arg("OVMF.fd");
        qemu.arg("-drive")
            .arg(format!("format=raw,file={}", uefi_path.display()));
    }

    qemu.arg("-m").arg("4096M");
    qemu.arg("-drive")
        .arg("file=target/test_disk.bin,format=raw,index=1,media=disk");
    qemu.arg("-netdev").arg("user,id=net0");
    qemu.arg("-device").arg("e1000,netdev=net0");
    qemu.arg("-serial")
        .arg("tcp:127.0.0.1:4444,server=on,wait=off,nodelay=on");
    qemu.arg("-rtc").arg("clock=host");

    for arg in extra_args {
        qemu.arg(arg);
    }

    let status = qemu.status().expect("Failed to execute QEMU");
    std::process::exit(status.code().unwrap_or(0));
}
