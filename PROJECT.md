# Project: Arctos OS ArcAI Architecture Overhaul

## Architecture Overview
Arctos OS bare-metal Rust kernel with native ArcAI integration, intent-driven fluid UI, zero-lag native component wiring engine, persistent memory, intermittent user checkups, and dynamic resource creation.

## Code Layout
- `src/main.rs`: Kernel entry point, system bootstrap, AIVFS mount, profile auto-load.
- `src/intent.rs`: Semantic intent parser, execution engine, intent traces, checkup triggers.
- `src/user_profile.rs`: OS-level preference memory, `UserProfile` struct, serialization to AIVFS `user_profile.cfg`, `CheckupEngine`.
- `src/apps/mod.rs`: Native component library (`UIWidgetKind`, `NativeGameKind`, physics canvas, Minesweeper, Snake, active app state).
- `src/model_loader.rs`: ArcAI bridge interface, zero-lag component wiring protocol parser, AIVFS disk cache (`cache_<app_name>.cfg`), native fallback generator.
- `src/bga.rs`: 1080p BGA graphics renderer, fluid UI surfaces, checkup banner rendering, theme color management.
- `src/fs/mod.rs` & `src/fs/structures.rs`: AIVFS filesystem, inode `semantic_tag_hash`, file read/write API.
- `scripts/ai_bridge.py`: Python host bridge streaming natural language requests to component-wiring protocol.
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

## Interface Contracts
### AI Bridge ↔ Kernel Component Wiring Protocol
- `APP:TITLE=<title>;WIDGET=panel,x,y,w,h,bg,border;WIDGET=text,...;WIDGET=engine,kind=minesweeper|snake|physics_bouncer`
- Zero-lag instant rendering without bytecode VM interpretation latency.

### User Profile Persistence Format (`user_profile.cfg` on AIVFS)
- Key-value properties: `username`, `role`, `ram_limit_mb`, `theme_primary`, `theme_secondary`, `active_surface`, `show_telemetry`, `is_first_boot`, 4 corner hotspot labels/actions.

### Resource Cache Format (`cache_<app_name>.cfg` on AIVFS)
- Cached raw component wiring spec string for instant zero-latency loading. Indexed via FNV-1a `semantic_tag_hash` in AIVFS Inode.

## Verification Status
- Microkernel target (`x86_64-arctos.json`): 0 errors
- SDK target (`arctos_sdk`): 0 errors
- Builder target (`arctos_builder`): 0 errors
- Host test suite: 5 passed, 0 failed
- Forensic Audit Verdict: CLEAN
