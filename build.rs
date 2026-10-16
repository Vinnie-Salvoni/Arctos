use std::fs::File;
use std::path::Path;

fn main() {
    // Ensure the target directory exists
    std::fs::create_dir_all("target").unwrap();
    
    // We create an empty raw disk image file of 10MB to act as our secondary drive
    let path = Path::new("target/test_disk.bin");
    if !path.exists() {
        let file = File::create(path).unwrap();
        // 10 MB = 10 * 1024 * 1024 bytes
        file.set_len(10 * 1024 * 1024).unwrap();
    }
    
    // Tell cargo to rerun this script only if build.rs itself changes
    println!("cargo:rerun-if-changed=build.rs");
}
