# Arctos OS — Development Guide & Command Reference

Welcome to the development guide for **Arctos OS**. This document maintains a clear record of key commands, environment setup notes, and development roadmap next steps.

---

## 🛠️ Key Commands Reference

Since the development environment is split between Windows (editing, Git, versioning) and WSL (compilation, running), here are the commands grouped by terminal.

### 🐧 Inside WSL (Ubuntu/Debian)
Use these commands inside your WSL terminal to build, check, and run the operating system in the QEMU emulator.

*   **Build the OS Image:**
    ```bash
    cargo bootimage
    ```
    *(Compiles the kernel, downloads/builds the bootloader, and creates `bootimage-arctos_os.bin` inside `/home/vinni/arctos_target`)*

*   **Run in QEMU:**
    ```bash
    cargo run
    ```
    *(Builds the OS and launches QEMU immediately to execute it)*

*   **Fast Compiler Check (No Image Creation):**
    ```bash
    cargo check
    ```

---

### 🪟 Inside Windows PowerShell (Host Machine)
Use these commands to manage version control (Git) and run helper scripts on your host machine.

*   **Auto-Increment the Project Version:**
    ```powershell
    powershell -ExecutionPolicy Bypass -File .\bump_version.ps1
    ```
    *(Increments the patch version in `Cargo.toml` automatically, e.g., `0.1.0` -> `0.1.1`)*

*   **Push Changes to GitHub:**
    ```powershell
    git add .
    git commit -m "Your description of changes"
    git push
    ```

---

## 🏗️ Project Architecture Overview

*   **`src/main.rs`**: Kernel entry point (`kernel_main`). Manages the boot sequence: clears screen, initializes GDT/TSS, IDT, PICs, paging, heap allocation, and runs the filesystem smoke test.
*   **`src/interrupts.rs`**: Defines the Interrupt Descriptor Table (IDT). Configures 8259 PIC offsets (IRQ 0-15 mapped to vectors 32-47). Handles timer (`INT 0x20`), keyboard (`INT 0x21`), and exceptions (breakpoint, double fault, page fault).
*   **`src/disk.rs`**: Low-level ATA PIO driver. Read/write operations poll the disk status register via `wait_ready`. Hardware interrupts from the disk controller are disabled by writing `0x02` to control port `0x3F6` to prevent conflicts.
*   **`src/fs/`**: Lightweight custom filesystem layout. Smoke tests formatting, mounting, creating files, reading files, listing directory files, and deletion.
*   **`src/gdt.rs`**: Global Descriptor Table & Task State Segment setup. Allocates an independent interrupt stack for handling Double Faults to prevent triple faults.
*   **`src/allocator.rs`**: Heap memory allocator configuration. Initializes `linked_list_allocator` for dynamic variables (`alloc::boxed::Box`, `alloc::vec::Vec`, etc.).
*   **`src/memory.rs`**: Kernel physical memory mapper. Uses the page tables set up by the bootloader to translate virtual addresses.

