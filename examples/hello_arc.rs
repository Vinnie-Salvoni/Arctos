#![no_std]
#![no_main]

use arctos_sdk::{arctos_main, KernelApi};

fn app(api: &KernelApi) {
    (api.clear_screen)(0);
    (api.draw_char)(24, 24, 'A' as u32, 15);
}

arctos_main!(app);
