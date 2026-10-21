# Arctos OS

**An experimental bare-metal x86_64 operating system in Rust, currently focused on kernel foundations and a text-mode shell.**

![Rust](https://img.shields.io/badge/language-Rust-orange?style=flat-square&logo=rust)
![no_std](https://img.shields.io/badge/no__std-bare--metal-blue?style=flat-square)
![Arch](https://img.shields.io/badge/arch-x86__64-lightgrey?style=flat-square)
![Status](https://img.shields.io/badge/status-early%20prototype-yellow?style=flat-square)
![License](https://img.shields.io/badge/license-PolyForm%20Noncommercial-red?style=flat-square)


---
![First ever Boot of the Project circa 26-06-26](<26-06-26 The Beginning.png>)
![mode 13h graphics test program](<2026-07-17 mode 13h graphics test.png>)
## The idea

Most operating systems organise interaction around apps, windows, menus, and icons. Arctos explores new approaches to operating-system interaction while prioritizing stable kernel foundations.

Arctos is an experimental, bare-metal x86_64 operating system written in Rust. The kernel currently boots to a keyboard-driven VGA text-mode shell. The project also explores connecting the kernel to a locally running language model through a host-side bridge.

The AI-driven experience is still a work in progress. Natural-language interpretation and structured UI actions are not yet fully implemented, and the in-kernel model inference code is a placeholder rather than a functioning language model.

Arctos is an early kernel project, not a replacement for a mature desktop operating system.

---

## What works today

| Area | Status |
| :--- | :--- |
| Boot in QEMU, x86_64 long mode | Working |
| Interrupts (IDT, dual 8259 PIC), PS/2 keyboard and mouse | Working |
| Paging, kernel heap | Working (small heap, about 100 KiB) |
| Host `.arc` packager | Prototype; validates ELF64/x86_64 load bounds, but the current format and kernel execution path are not isolated |
| Rust Cargo tooling | Host `cargo arc test` runs standard Rust projects; `cargo arc build` still targets the `no_std` SDK and legacy `.arc` format |
| Kernel `.arc` execution | Experimental legacy path runs with kernel privilege; do not run untrusted `.arc` files |
| Host AIVFS image tool | Reads and writes the kernel's current inode layout with disk/file bounds checks |
| Host-side LLM bridge over serial | Basic integration; requires the bridge script and a compatible local model setup |
| In-kernel model loading and inference (`model_loader.rs`) | Placeholder only, not real inference |
| Intel Level Zero / GPU acceleration (`ai/`) | Stubs only |

If a feature is not listed as working, assume it does not work yet.

---

## How it works

```
 You type a command
        |
        v
 +--------------+
 | Arctos kernel|
 | text shell   |
 +--------------+
```

The kernel shell remains the primary interface. The project may explore
connecting host-side language-model inference over a serial connection later.

The next step is to define a strict, validated action protocol, for example `{"action": "set_theme", "value": "cyan"}`. The kernel should accept only explicitly supported actions and validate their parameters before changing the interface. Model output should never be treated as executable instructions.

In the longer term, Arctos aims to explore how intent-driven interaction can become a core part of an operating system, while keeping inference local to the user's machine.

The kernel itself is monolithic: drivers, graphics, and the intent logic all run in kernel space.

---

## Design goals

- **Kernel first:** stabilize boot, memory, interrupts, and the text-mode shell before adding higher-level interfaces.
- **Local first:** the model runs on your hardware.
- **Separation of concerns:** keep host-side inference separate from kernel command handling.

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

This builds the kernel for `x86_64-arctos.json`, creates a bootable image with
`bootimage`, and launches QEMU into the text-mode command shell.

### Validate changes

The repository defaults to the custom bare-metal target. Check the kernel
separately from host-side packages, and use the host target reported by
`rustc -vV` for workspace tests:

```bash
cargo fmt --all -- --check
cargo check -p arctos_os
cargo check -p arctos_sdk
cargo test --workspace --exclude arctos_os --exclude arctos_sdk --target <host-target>
cargo test -p arctos_builder --target <host-target>
python -m unittest scripts.test_ai_bridge
```

### Build a Rust `.arc` application

The host builder provides an early Cargo subcommand for compiling either a
single Rust source file or a Cargo application project against the `no_std` SDK
and packaging the resulting ELF:

```bash
cargo install --path builder --target <host-target>
cargo arc build examples/hello_arc.rs -o hello.arc
cargo arc build examples/arc_hello -o hello-project.arc
cargo arc inspect hello.arc
cargo arc test examples/rust_playground
```

Replace `<host-target>` with the `host:` value printed by `rustc -vV`; the
workspace's default target is bare metal and cannot run the host-side tool.
The build uses the repository's nightly custom target and requires `rust-src`.

Applications use `#![no_std]`, `#![no_main]`, the `arctos_sdk` crate and its
`arctos_main!` entry macro. See [examples/hello_arc.rs](./examples/hello_arc.rs).
For Cargo projects, the package name must match the executable target name; see
[examples/arc_hello/](./examples/arc_hello).
For ordinary Rust programs using `std`, see
[examples/rust_playground/](./examples/rust_playground). ArcStudio saves the
current file to the AIVFS image; extract it into a host Cargo project and test
it with:

```bash
cargo arc extract target/test_disk.bin main.rs -o examples/rust_playground/src/main.rs
cargo arc test examples/rust_playground
```

`cargo arc test` uses the host toolchain and target; it does not imply standard
Rust programs run inside the kernel.
This pipeline currently targets the legacy `.arc` layout; it does **not** make
applications safe to execute. The current kernel entry path runs app code with
kernel privilege, so only run code you wrote and trust. A versioned container,
relocations, and isolated user processes must be implemented before third-party
apps can be safely launched.

Kernel builds disable `.arc` execution by default. Re-enabling the old
kernel-privileged loader requires the explicit `legacy-ring0-apps` Cargo feature
and is unsafe for untrusted binaries.

### Optional: connect a local model

Install Ollama and pull a model on the host first (the bridge prefers
`qwen2.5-coder:7b`, then `qwen2.5:7b`):

```bash
# Requires Ollama (https://ollama.com)
ollama pull qwen2.5-coder:7b
```

Start the bridge, then start QEMU in another terminal:

```bash
python scripts/ai_bridge.py
```

The bridge connects to the QEMU COM1 TCP socket at `127.0.0.1:4444`, matching the
serial settings in `Cargo.toml`. It uses Ollama's local HTTP API and needs no
additional Python packages. If another model is already installed, it is selected
automatically; choose one explicitly with `--model <name>`. The Ollama URL and
serial host/port can also be set with `--ollama-url`, `--serial-host`, and
`--serial-port`. Ollama output is read as it streams, and its per-read idle
timeout defaults to ten minutes; adjust it with `--ollama-timeout <seconds>` if
your machine needs longer. This is an idle timeout between streamed chunks. The
OS shows a processing indicator and reports an error if the full request receives
no serial reply within 30 minutes.

On slower machines, try a smaller installed model explicitly (for example,
`python scripts/ai_bridge.py --model qwen2.5:1.5b`) for faster responses; smaller
models may follow the app protocol less reliably than the preferred coder model.

The intent bar routes prompts through the local model. App and game requests use
the kernel's validated app protocol: simple interfaces can use widgets, while
custom apps can use the bounded drawing/state VM. The model receives the actual
VM instruction set and app canvas bounds; no app-specific templates are built
into the OS. App content is confined to the 1920x976 area between the title bar
and status footer, and the system hides its intent/response overlays while an app
is open. Press Escape or click `[X]` to close the app. Generated apps are
experimental; unsupported instructions or out-of-bounds layouts are rejected
with an error message. Game requests are also checked for basic input, update,
and drawing operations; Snake requests receive additional structural checks. The
bridge asks the model to repair invalid output once, then reports an error if it
still fails validation. These checks verify protocol structure, not that the
resulting game is fully playable.
Background requests support up to four colors and solid, gradient, wave, spiral,
rings, stripes, grid, and dot patterns (for example, `blue, purple, and cyan
spiral wallpaper`). Simple background changes are applied directly by the
kernel; the bridge also keeps the last ten prompt/reply turns so follow-up
requests such as `make it more blue` have context.

---

## Controls

The kernel accepts keyboard input in the shell. Type a command and press
**Enter**; use `help` to see the available commands.

---

## Project structure

```
Arctos/
├── src/
│   ├── main.rs           # Kernel entry and boot sequence
│   ├── shell.rs          # Shell loop and command execution
│   ├── interrupts.rs     # IDT and IRQ handlers
│   ├── memory.rs         # Paging and frame allocator
│   ├── allocator.rs      # Kernel heap
│   ├── fs/               # AIVFS virtual filesystem
│   ├── ai/               # AI and hardware prototypes
├── scripts/
│   └── ai_bridge.py      # Host-side serial-to-LLM bridge
├── x86_64-arctos.json    # Custom bare-metal target spec
└── Cargo.toml
```

---

## Roadmap

The checklist below is ordered by dependency, not by the feature numbers in the
original feature request. A checked item means the capability is implemented
and verified to the stated level; prototypes and design work remain unchecked.
The kernel currently starts in text mode while its boot and shell foundations
are stabilized.

### 1. Stabilize the kernel and shell

- [ ] Verify boot, keyboard input, and shell behavior in repeatable QEMU runs
- [ ] Add focused tests for shell command parsing and device input

### 2. Establish safe processes and a real `.arc` application platform

- [x] Harden host packager checks for ELF64/x86_64 headers, program-table bounds, segment bounds, executable entry points and a 16 MiB image limit
- [x] (2) Package and inspect single-file and manifest-based Rust SDK applications using the early `cargo arc build/inspect` subcommand
- [x] Provide host-side standard Rust `cargo arc test` and bounded AIVFS extraction for an edit-and-test workflow
- [x] Make the host disk-image builder read/write the kernel's 64-byte AIVFS inode layout and reject out-of-range disk/file extents
- [ ] (2) Add reproducible build metadata, project resource sections, and support for multiple named binary targets
- [x] (3) Disable the unsafe legacy ring-0 `.arc` loader by default; keep explicit opt-in only for trusted development binaries
- [ ] (3) Specify the native Rust `std` target and Arctos platform ABI: process startup/exit, syscalls, file descriptors, environment, console I/O, allocation, time, entropy, threads/TLS, synchronization, and panic behavior
- [ ] (3) Add a `rustc`/Cargo target and reproducible `std` build pipeline for Arctos; do not treat host `cargo test` or the existing `no_std` SDK as native `std` support
- [ ] (3) Define privilege separation, scheduler/context switching, per-process address spaces, W^X memory permissions, kernel/user transitions, IPC and process resource limits
- [ ] (3) Implement and test ring-3 entry/return, fault containment, process creation/exit and resource cleanup before allowing third-party code to execute
- [ ] (3) Add a bounds-checked ELF loader with relocations, executable/read-only/writable segment permissions, user stack and ABI-compatible process startup
- [ ] (3) Implement a validated syscall boundary and the kernel services required by the Arctos `std` backend, including console, VFS file descriptors, clock, entropy, process lifecycle and memory mapping
- [ ] (3) Implement and test the `std` platform backend and required runtime support (including TLS and synchronization); explicitly document any unavailable `std` APIs
- [ ] (3) Run statically linked Arctos-targeted `std` programs in isolated processes; add end-to-end tests for `println!`, `String`/`Vec`, file I/O, arguments, time, threads, panic handling and clean exit
- [ ] (1) Specify a versioned `.arc` container with metadata, checked section tables, entry point, dynamic symbols, permissions and resource sections
- [x] (2) Provide the early `no_std` Rust SDK and `cargo arc build/inspect` workflows; standard-library support and reproducible release metadata remain future work
- [ ] (3) Replace the legacy container/loader with the versioned format only after process isolation and the Rust runtime are verified
- [ ] Remove or gate the legacy ring-0 `.arc` execution path until isolation and the new loader are complete
- [ ] Add sample applications and compatibility/version tests for the container, compiler, loader and SDK

### 3. Build ArcStudio on the application and file APIs

- [x] Add bounded character-level undo/redo history to ArcStudio; grouped editing, selection and clipboard remain future work
- [ ] (4) Extend ArcStudio beyond its single-document editing/save prototype with tabs, bracket matching, full Rust/`.arc` syntax parsing and a structured document model
- [ ] (7) Add a project tree, file create/delete, project-wide search/replace and file-operation error handling
- [ ] (5) Integrate Rust Analyzer/LSP over a documented host service or an OS-supported process API; provide diagnostics, completion, navigation, signatures and refactoring
- [ ] (6) Add an embedded terminal/build console for `cargo arc build/run`, streamed diagnostics, logs and process launch output
- [ ] (8) Add an interactive UI designer/preview backed by the same stable app UI API
- [ ] Validate the complete edit → build → install/run → debug loop with bundled sample projects

### 4. Add the remaining native applications and system services

- [ ] (22) Add an offline Markdown/HTML documentation viewer with bounded parsing and no implied web access
- [ ] (23) Implement a network stack and permission model before exposing sockets; then add a live, read-only traffic/connection visualizer
- [ ] (21) Show per-process resource graphs once processes have isolated identities and accounting
- [ ] Give every bundled application storage, lifecycle, keyboard-accessibility and failure/recovery tests

### 5. Keep AI integration bounded and verifiable

- [ ] Validate assistant output as typed, explicitly supported proposals; never interpret model text as shell input or executable code
- [ ] Require clear user confirmation and permission checks for any action that changes files or system settings
- [ ] Remove placeholder inference; use the host bridge until a real, measured inference and driver path is available

### 6. Release and hardening gates

- [ ] Fuzz `.arc`, ELF, image, document, network and UI-protocol parsers with strict memory/resource limits
- [ ] Harden allocation failures, interrupt/heap initialization, panic recovery and disk/filesystem error handling
- [ ] Publish a demo and design notes describing the app ABI, security model, graphics limits and measured performance
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
