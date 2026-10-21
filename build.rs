use std::fs::File;
use std::path::Path;

fn main() {
    // Ensure the target directory exists
    std::fs::create_dir_all("target").unwrap();

    // Create the empty raw disk images attached by the QEMU runner.
    for name in ["test_disk.bin", "model_disk.bin"] {
        let path = Path::new("target").join(name);
        if !path.exists() {
            let file = File::create(path).expect("failed to create QEMU disk image");
            file.set_len(10 * 1024 * 1024)
                .expect("failed to size QEMU disk image");
        }
    }

    // Tell cargo to rerun this script only if build.rs itself changes
    println!("cargo:rerun-if-changed=build.rs");
}
