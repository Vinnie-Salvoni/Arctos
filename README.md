# Arctos OS

**An experimental bare-metal x86_64 operating system in Rust, exploring an intent-driven interface: you describe what you want, and the screen reshapes itself.**

![Arctos demo](docs/demo.gif)
<!-- Replace with a 20-30 second screen recording of the intent bar changing the UI. -->

![Rust](https://img.shields.io/badge/language-Rust-orange?style=flat-square&logo=rust)
![no_std](https://img.shields.io/badge/no__std-bare--metal-blue?style=flat-square)
![Arch](https://img.shields.io/badge/arch-x86__64-lightgrey?style=flat-square)
![Status](https://img.shields.io/badge/status-early%20prototype-yellow?style=flat-square)
![License](https://img.shields.io/badge/license-PolyForm%20Noncommercial-red?style=flat-square)

---
![alt text](<26-06-26 The Beginning.png>)
![alt text](<2026-07-17 mode 13h graphics test.png>)
## The idea

Most operating systems still present a grid of icons, windows, and menus. Arctos asks a different question: what if the interface were a single fluid surface that reshapes itself around what you are trying to do?

You type (and eventually speak) an intent into a floating bar. A local language model interprets it, and the kernel updates the interface to match. Everything runs on your own machine, with no cloud involved.

This is an early prototype built by one person. It proves the loop works end to end. It is not yet the finished vision.

---

## What works today

| Area | Status |
| :--- | :--- |
| Boot in QEMU, x86_64 long mode | Working |
| 1080p 32bpp BGA framebuffer rendering (wallpaper gradients, cards, overlays) | Working |
| Interrupts (IDT, dual 8259 PIC), PS/2 keyboard and mouse | Working |
| Paging, kernel heap | Working (small heap, about 100 KiB) |
| Intent bar with scrollable response card | Working |
| Local LLM bridge (host-side script relays prompts and replies over serial) | Working, basic |
| Intents that change the UI: theme, background, workspace, telemetry, hotspots | Working (keyword-based) |
| Persistent user profile (theme, hotspots, workspace) via AIVFS | Working |
| Natural-language intent to structured UI action | In progress |
| In-kernel model loading and inference (`model_loader.rs`) | Placeholder only, not real inference |
| Intel Level Zero / GPU acceleration (`ai/`) | Stubs only |

If a feature is not listed as working, assume it does not work yet.

---

## How it works

```
 You type an intent
        |
        v
 +--------------+   serial    +------------------+
 | Arctos kernel| <---------> | Host: ai_bridge  |
 | (intent bar) |             | (Ollama / LLM)   |
 +------+-------+             +------------------+
        |
        v
 Kernel maps the reply to a UI state change
        |
        v
 BGA renderer redraws the surface
```

The language model runs on the host machine, not inside the kernel. Arctos sends your prompt over a serial connection, receives the model's reply, and applies it to the interface. The long-term goal is a strict, validated action format (for example `{"action": "set_theme", "value": "cyan"}`) so that model output can never do more than an allowlist permits.

The kernel itself is monolithic: drivers, graphics, and the intent logic all run in kernel space.

---

## Design goals

- **Intent over navigation:** describe the outcome, not the menu path.
- **No legacy desktop tropes:** no taskbar, start menu, or titlebar-first windows.
- **Local first:** the model runs on your hardware.
- **Separation of concerns:** the renderer (`bga.rs`) is decoupled from the intent logic. State is injected, and the canvas reacts.

---

## Build and run

### Prerequisites

```bash
# Rust nightly toolchain
rustup toolchain install nightly
rustup component add rust-src llvm-tools-preview --toolchain nightly

# bootimage
cargo install bootimage

# QEMU
# Windows: winget install qemu  (or https://www.qemu.org/download/)
# Linux:   sudo apt install qemu-system-x86
```

### Run in QEMU

```bash
cargo run
```

This builds the kernel for `x86_64-arctos.json`, creates a bootable image with `bootimage`, and launches QEMU at 1920x1080 in BGA graphics mode.

### Optional: connect a local model

```bash
# Requires Ollama (https://ollama.com) or any OpenAI-compatible endpoint
python scripts/ai_bridge.py
```

Then run `cargo run`. The intent bar will route prompts through your local model.

---

## Controls

Everything is driven from the **`INTENT >`** bar at the bottom of the screen.

| Input | Action |
| :--- | :--- |
| Type + `Enter` | Submit an intent |
| `Backspace` | Delete last character |
| `PageUp` / `Up` | Scroll the response card up |
| `PageDown` / `Down` | Scroll the response card down |
| Mouse click | Interact with hotspots and cards |

### Example intents

```
dev workspace          -> developer telemetry surface
tensor workspace       -> AI tensor workspace surface
telemetry on           -> kernel memory and interrupt overlay
theme cyan             -> cyan spectral wallpaper
background purple      -> purple wallpaper
set hotspot 40 40 Dev  -> pin a persistent hotspot at (40, 40)
```

---

## Project structure

```
Arctos/
├── src/
│   ├── main.rs           # Kernel entry and boot sequence
│   ├── bga.rs            # BGA graphics engine and UI rendering
│   ├── model_loader.rs   # Intent engine hook (placeholder inference)
│   ├── shell.rs          # Intent shell loop
│   ├── interrupts.rs     # IDT and IRQ handlers
│   ├── memory.rs         # Paging and frame allocator
│   ├── allocator.rs      # Kernel heap
│   ├── fs/               # AIVFS virtual filesystem
│   ├── ai/               # Hardware context stubs
│   ├── apps.rs           # App spawning
│   └── user_profile.rs   # Persistent profile and hotspots
├── scripts/
│   └── ai_bridge.py      # Host-side serial-to-LLM bridge
├── x86_64-arctos.json    # Custom bare-metal target spec
└── Cargo.toml
```

---

## Roadmap

- [ ] Structured, validated intent protocol (replace keyword matching)
- [ ] Natural-language intents that reshape the layout, not just the colours
- [ ] Remove placeholder inference; keep inference on the host until a real in-kernel or driver path exists
- [ ] Demo video and a written design note on the intent model
- [ ] Hardening: panic handler, model-file parsing, interrupt/heap init order

---

## Acknowledgements

The kernel foundation (boot setup, interrupts, paging, heap) builds on ideas and crates from Philipp Oppermann's excellent [Writing an OS in Rust](https://os.phil-opp.com/) series. The intent-driven interface, rendering engine, AIVFS, and AI bridge are my own work in progress.

Dependencies are all MIT or Apache-2.0: `volatile`, `spin`, `x86_64`, `pic8259`, `pc-keyboard`, `linked_list_allocator`, `bootloader`, `lazy_static`.

---

## License

Arctos is **source-available, not open source.** It is licensed under the [PolyForm Noncommercial License 1.0.0](./LICENSE).

You may read it, run it, and experiment with it for personal, educational, and research purposes. You may not use it commercially or build a product from it without a separate agreement.

Copyright (c) 2026 Vinnie Salvoni.

## Contributing

I'm not accepting pull requests for now. Bug reports, questions, and ideas are very welcome as GitHub Issues or Discussions.

## Get in touch

I'm looking for people interested in bare-metal Rust, local AI, novel interface design, or product design. If you tried the demo and have thoughts, or want to collaborate:

**[Vinnie-Salvoni@users.noreply.github.com]**