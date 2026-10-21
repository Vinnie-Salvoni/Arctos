use std::fs::File;
use std::path::Path;

fn main() {
    // Ensure the target directory exists.
    std::fs::create_dir_all("target").unwrap();

    // Create the raw disk image used by the QEMU runner.
    let path = Path::new("target").join("test_disk.bin");

    if !path.exists() {
        let file = File::create(path).expect("failed to create QEMU disk image");
        file.set_len(10 * 1024 * 1024)
            .expect("failed to size QEMU disk image");
    }

    println!("cargo:rerun-if-changed=build.rs");
}
