# Project: Arctos OS ArcAI Architecture Overhaul

## Architecture Overview
Arctos OS is a bare-metal Rust kernel currently focused on boot, memory, interrupts, device input, and a text-mode shell.

## Code Layout
- `src/main.rs`: Kernel entry point, system bootstrap, and AIVFS mount.
- `src/shell.rs`: Keyboard-driven text-mode command shell.
- `src/interrupts.rs`: IDT and hardware interrupt handlers.
- `src/memory.rs`: Physical frame allocator and paging.
- `src/fs/`: AIVFS filesystem.
- `scripts/ai_bridge.py`: Host-side serial bridge prototype.
- `sdk/`: Arctos SDK interfaces.
- `builder/`: Standalone `.arc` app builder tool.
- `.agents/`: Agent metadata workspace.

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| 1 | Architecture Audit & Exploration | Comprehensive audit of codebase, Cargo.toml, src/ layout, AI bridge | None | DONE |
| 2 | Milestone 1: R1 Rich Component Library | Build fix (`close_app`), `UIWidgetKind` extension, zero-lag component wiring protocol | M1 | DONE |
| 3 | Milestone 2: R2 OS-Level Preference Memory | `UserProfile` expansion, `user_profile.cfg` persistence, boot auto-load, theme/layout restoration | M1 | DONE |
| 4 | Milestone 3: R3 Intermittent User Checkups | `CheckupEngine`, first-boot & periodic checkup prompts, conversational preference refinement | M2, M3 | DONE |
| 5 | Milestone 4: R4 Dynamic Resource Creation | AIVFS disk cache (`cache_<app_name>.cfg`), bare-metal fallback generator, `semantic_tag_hash` | M2, M3 | DONE |
| 6 | Phase 6: E2E Verification & Forensic Audit | Reviewers, Challengers, Forensic Auditor verification | M2, M3, M4, M5 | DONE |

## Verification Status
- Microkernel target (`x86_64-arctos.json`): 0 errors
- SDK target (`arctos_sdk`): 0 errors
- Builder target (`arctos_builder`): 0 errors
- Host test suite: 5 passed, 0 failed
- Forensic Audit Verdict: CLEAN
