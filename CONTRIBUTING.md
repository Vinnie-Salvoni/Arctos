# Contributing to Arctos OS 🌌

First off — thanks for being here. Arctos is a small project with a big vision, and every contributor matters.

This document explains how to get set up, how the codebase is structured, and how to contribute without stepping on anyone's toes.

---

## 📐 Project Philosophy

Before diving in, please read the short philosophy section in the [README](./README.md). The TL;DR:

- Arctos is **not** a traditional OS clone. Don't introduce taskbars, start menus, or rigid window management as primary interface elements.
- The UI surface is **intent-driven** — it reacts to natural language and morphs around the user.
- **AI is a kernel primitive** here, not a userspace app. The intent hook lives in the kernel.
- Code must be `#![no_std]` compatible. No standard library, no OS beneath us.

---

## 🏛️ Code Ownership / Domain Split

The codebase has a deliberate split in responsibility:

| Domain | Files | Owner |
| :--- | :--- | :--- |
| **Visual rendering & UI aesthetics** | `src/bga.rs` | Lead Designer |
| **Kernel internals, memory, interrupts, drivers** | everything else in `src/` | Systems / CTO |
| **AI intent engine** | `src/model_loader.rs` (hook) | AI / Systems |
| **Host-side tooling** | `scripts/` | Open |

If you're working on UI, stay in `bga.rs`. If you're working on systems, don't redesign the canvas renderer. The interface contract between them is the global state variables in `bga.rs` (e.g. `CURRENT_THEME`, `GENERATIVE_CANVAS_PARAMS`).

---

## 🚀 Getting Set Up

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

## 🌿 Branching & PRs

- Branch off `main` with a descriptive name: `feature/bga-glassmorphism`, `fix/ps2-mouse-scroll`, `docs/contributing`
- Keep PRs focused. One concern per PR.
- Run `cargo check` before submitting — zero compile errors required.
- Warnings are okay for now (we have a known set), but don't add new ones if avoidable.

---

## 🔑 Key Files to Know

```
src/
├── main.rs           # Kernel entry point — boot sequence, subsystem init
├── bga.rs            # BGA graphics engine — ALL visual rendering lives here
│                     #   Key globals: CURRENT_THEME, SECONDARY_THEME,
│                     #   TERTIARY_THEME, GENERATIVE_CANVAS_PARAMS,
│                     #   CURRENT_SURFACE, FORCE_REDRAW
├── model_loader.rs   # Intent engine hook — plug your AI backend in here
├── shell.rs          # The main input loop — reads keystrokes, calls intent engine
├── interrupts.rs     # IDT handlers for CPU exceptions & hardware IRQs
├── memory.rs         # Physical frame allocator & page table init
├── allocator.rs      # Kernel heap (linked list allocator)
├── fs/mod.rs         # AIVFS — lightweight in-kernel virtual filesystem
├── user_profile.rs   # Persistent user preferences & hotspot anchors
├── apps.rs           # App spawning framework — parse "APP:X" intents
├── mouse.rs          # PS/2 mouse driver
└── ai/               # Intel Level Zero hardware context stubs
```

---

## 🧱 The UI Contract

If you're building something that changes the visual surface, **don't call rendering functions directly**. Instead, mutate the global state variables that the `repaint_canvas` loop in `bga.rs` already watches:

```rust
// Change the primary wallpaper colour
*crate::bga::CURRENT_THEME.lock() = 0x00_FF_88; // RGB hex

// Set a secondary blended colour
*crate::bga::SECONDARY_THEME.lock() = Some(0xFF_00_88);

// Force the canvas to redraw on next tick
crate::bga::FORCE_REDRAW.store(true, core::sync::atomic::Ordering::Relaxed);
```

To spawn an application from the intent engine, return the string `"APP:AppName"` from `process_intent`.

---

## 💬 Discussions & Questions

Open a [GitHub Discussion](https://github.com/Vinnie-Salvoni/Arctos/discussions) for:
- Architecture questions before writing code
- Ideas for new intent surface paradigms
- Questions about the hardware target (Intel Core Ultra 7 255H / Arc 140T)

Open an [Issue](https://github.com/Vinnie-Salvoni/Arctos/issues) for:
- Bugs you've found
- Features you want to build (so we can coordinate)

---

## ✅ Code Style

- Standard `rustfmt` formatting (`cargo fmt`)
- Keep `unsafe` blocks minimal and clearly commented
- Preserve all existing doc comments — add to them, don't delete
- No `std` — if you need something, check `alloc` first

---

Thanks again. Let's build something people haven't seen before. 🚀
