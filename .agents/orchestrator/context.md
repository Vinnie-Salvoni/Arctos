# Context — ArcAI Architecture Overhaul

## Project Background & Goal
Overhaul the Arctos OS ArcAI Architecture to enable dynamic generation of apps, games, or documents with minimal lag, utilizing a persistent memory system and native component wiring.

## User Requirements & Acceptance Criteria

### R1. Rich Component Library (Zero-Lag Execution)
- Implement optimized pre-built native modules within the kernel (UI widgets, physics engines, game states like Minesweeper/Snake).
- AI bridge dynamically configures and wires pre-built native components rather than generating raw bytecode per frame.
- Natural language requests are translated into structured component-wiring protocols.

### R2. OS-Level Preference Memory
- Persistent memory system storing user preferences, custom interface layouts, and conversational context to an OS-level file (or host bridge simulation).
- Automatically loads preferences on boot.
- Restores custom themes/layouts when requested (e.g., "load my preferred interface").

### R3. Intermittent User Checkups
- Proactive, unobtrusive interface verification with user during initial use to build and refine user profile.

### R4. Dynamic Resource Creation
- When an requested resource/component is missing, dynamically generate and persist it in memory for future reuse.

## Core Architectural Invariants & Rules (AGENTS.md)
- Microkernel in Rust (bare-metal x86_64).
- Intent-Driven UI Concept: fluid visual surface listening for intent, morphing views.
- No legacy window manager tropes (no titlebars, taskbars, start menus as primary interfaces).
- Bare-Metal VGA Engineering rules if graphics/text mode switching.
- First-Boot Hotspot Guidance & Conversational UI Adaptability.
- Real Semantic Intent Engine & Autonomous Self-Improvement Loop.
