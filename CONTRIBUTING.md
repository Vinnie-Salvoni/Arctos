# Contributing to Arctos OS

This document explains how to get set up, how the codebase is structured, and how to contribute without stepping on anyone's toes.

---

## Project Philosophy

Before diving in, please read the short philosophy section in the [README](./README.md). The TL;DR:

- The kernel currently boots to a keyboard-driven text-mode shell; prioritize stable kernel foundations.
- The kernel is `#![no_std]`; host-side tools such as the builder and AI bridge may use standard libraries.

---


## Getting Set Up

### 1. Clone the repo

```bash
git clone https://github.com/Vinnie-Salvoni/Arctos.git
cd Arctos
```

### 2. Install the Rust nightly toolchain

```bash
rustup toolchain install nightly
rustup component add rust-src llvm-tools-preview --toolchain nightly
cargo install bootimage
```

### 3. Install QEMU

- **Windows:** `winget install qemu` or download from [qemu.org](https://www.qemu.org/download/)
- **Linux:** `sudo apt install qemu-system-x86`
- **macOS:** `brew install qemu`

### 4. Build and run

```bash
cargo run
```

That's it. The kernel compiles, `bootimage` wraps it in a disk image, and QEMU boots it at 1080p.

---

## Validation

Run formatting and the checks relevant to the files you changed:

```bash
cargo fmt --all -- --check
cargo check -p arctos_os
cargo check -p arctos_sdk
cargo test --workspace --exclude arctos_os --exclude arctos_sdk --target <host-target>
python -m unittest scripts.test_ai_bridge
```

Use the host target reported by `rustc -vV` for the host workspace tests. The
repository's Cargo configuration selects the custom bare-metal target by
default, so host packages need an explicit host target. The kernel and `no_std`
SDK are checked separately rather than built with the host test harness.

## Branching & PRs

- Branch off `main` with a descriptive name, such as `feature/intent-layout` or `fix/ps2-mouse-scroll`.
- Keep changes focused and run the relevant validation commands before submitting.
- Existing kernel warnings are tracked; avoid introducing new warnings.

## Key Files to Know

```
src/
├── main.rs             # Kernel entry point and boot sequence
├── shell.rs            # Keyboard input and command execution
├── fs/                 # In-kernel virtual filesystem
├── memory.rs           # Physical frame allocator and paging
├── interrupts.rs       # CPU exceptions and hardware IRQs
├── mouse.rs            # PS/2 mouse driver
└── ai/                 # AI and Level Zero hardware prototypes
scripts/
└── ai_bridge.py        # Host-side serial-to-model bridge
```

---

## Discussions & Questions

Open a [GitHub Discussion](https://github.com/Vinnie-Salvoni/Arctos/discussions) for:
- Architecture questions before writing code
- Ideas for new intent surface paradigms
- Questions about the hardware target (Intel Core Ultra 7 255H / Arc 140T)

Open an [Issue](https://github.com/Vinnie-Salvoni/Arctos/issues) for:
- Bugs you've found
- Features you want to build (so we can coordinate)

---

## Code Style

- Standard `rustfmt` formatting (`cargo fmt`)
- Keep `unsafe` blocks minimal and clearly commented
- Preserve all existing doc comments — add to them, don't delete
- No `std` — if you need something, check `alloc` first

---

Thanks again. Let's build something people haven't seen before. 🚀
