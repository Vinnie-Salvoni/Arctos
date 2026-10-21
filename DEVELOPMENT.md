# Arctos OS Development Guide

Arctos combines a bare-metal kernel with host-side Rust tools and a Python AI
bridge. The root Cargo configuration selects the custom `x86_64-arctos` target
by default; pass an explicit host target when building or testing host packages.

## Build and run

Install the nightly toolchain with the `rust-src` and `llvm-tools-preview`
components, `bootimage`, and QEMU. Then run:

```bash
cargo check -p arctos_os
cargo run
```

`cargo run` builds the kernel image and launches it in QEMU using the arguments
in `Cargo.toml`.

## Validation

```bash
cargo fmt --all -- --check
cargo check -p arctos_os
cargo check -p arctos_sdk
cargo test --workspace --exclude arctos_os --exclude arctos_sdk --target <host-target>
python -m unittest scripts.test_ai_bridge
```

Find the Rust host target with `rustc -vV`. The bare-metal kernel and `no_std`
SDK are checked separately because they use the custom target and cannot use
the host test harness.

## Project layout

- `src/main.rs`: kernel entry point and initialization
- `src/shell.rs`: keyboard-driven text-mode shell
- `src/fs/`: in-kernel virtual filesystem
- `src/memory.rs`, `src/interrupts.rs`: memory management and hardware IRQs
- `scripts/ai_bridge.py`: host-side serial-to-model bridge
- `builder/`, `sdk/`, `crates/`: host builder, app SDK, and Rust tools
