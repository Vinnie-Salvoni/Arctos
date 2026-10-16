#![no_std]

#[macro_export]
macro_rules! arctos_main {
    ($path:path) => {
        #[no_mangle]
        #[link_section = ".text.app_entry"]
        pub extern "C" fn app_entry(api: &$crate::KernelApi) {
            let entry: fn(&$crate::KernelApi) = $path;
            entry(api);
        }

        #[panic_handler]
        fn panic(_info: &core::panic::PanicInfo) -> ! {
            loop {}
        }
    };
}

#[repr(C)]
pub struct KernelApi {
    pub draw_char: extern "C" fn(usize, usize, u32, u8),
    pub draw_rect: extern "C" fn(usize, usize, usize, usize, u8),
    pub draw_rect_filled: extern "C" fn(usize, usize, usize, usize, u8),
    pub clear_screen: extern "C" fn(u8),
    pub swap_buffers: extern "C" fn(),
    pub read_key: extern "C" fn() -> u32,
    pub write_file: extern "C" fn(*const u8, usize, *const u8, usize) -> bool,
    pub read_file: extern "C" fn(*const u8, usize, *mut u8, usize) -> usize,
    pub sleep: extern "C" fn(usize),
    pub get_cmdline: extern "C" fn(*mut u8, usize) -> usize,
    pub list_files: extern "C" fn(*mut u8, usize) -> usize,
    pub delete_file: extern "C" fn(*const u8, usize) -> bool,
    pub get_ticks: extern "C" fn() -> u64,
}

// Compiler helper definitions for standalone apps
#[no_mangle]
#[allow(suspicious_runtime_symbol_definitions)]
pub unsafe extern "C" fn memset(s: *mut u8, c: i32, n: usize) -> *mut u8 {
    core::arch::asm!(
        "cld",
        "rep stosb",
        inout("rcx") n => _,
        inout("rdi") s => _,
        in("al") c as u8,
    );
    s
}

#[no_mangle]
#[allow(suspicious_runtime_symbol_definitions)]
pub unsafe extern "C" fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    core::arch::asm!(
        "cld",
        "rep movsb",
        inout("rcx") n => _,
        inout("rdi") dest => _,
        inout("rsi") src => _,
    );
    dest
}

#[no_mangle]
#[allow(suspicious_runtime_symbol_definitions)]
pub unsafe extern "C" fn memcmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    let mut result: i32;
    core::arch::asm!(
        "cld",
        "repe cmpsb",
        "seta al",
        "setb dl",
        "movzx eax, al",
        "movzx edx, dl",
        "sub eax, edx",
        inout("rcx") n => _,
        inout("rsi") s1 => _,
        inout("rdi") s2 => _,
        out("eax") result,
        out("edx") _,
    );
    result
}
