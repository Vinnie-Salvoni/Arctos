use std::fs::{self, File};
use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo did not set OUT_DIR"));

    let kernel = PathBuf::from(
        std::env::var_os("CARGO_BIN_FILE_KERNEL_kernel")
            .expect("Cargo did not provide the compiled kernel artifact"),
    );

    // Create the bootable BIOS disk image.
    let bios_path = out_dir.join("arctos-bios.img");

    bootloader::BiosBoot::new(&kernel)
        .create_disk_image(&bios_path)
        .expect("failed to create the Arctos BIOS image");

    // Keep the separate 10 MiB data disk used by Arctos's filesystem.
    let manifest_dir = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("Cargo did not set CARGO_MANIFEST_DIR"),
    );

    let target_dir = manifest_dir.join("target");
    fs::create_dir_all(&target_dir).expect("failed to create the target directory");

    let test_disk = target_dir.join("test_disk.bin");

    if !test_disk.exists() {
        let disk = File::create(&test_disk).expect("failed to create the QEMU test disk");

        disk.set_len(10 * 1024 * 1024)
            .expect("failed to size the QEMU test disk");
    }

    // Pass the generated image location to the launcher binary.
    println!("cargo:rustc-env=BIOS_PATH={}", bios_path.display());

    println!("cargo:rerun-if-changed=build.rs");
}
