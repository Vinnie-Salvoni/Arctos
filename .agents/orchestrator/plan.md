# Plan — ArcAI Architecture Overhaul

## Overview
Decompose the ArcAI overhaul into phased milestones, starting with initial codebase exploration and architectural mapping, followed by native component library implementation, persistence memory, checkups, dynamic resource creation, and comprehensive E2E verification.

## Milestones

### Phase 1: Exploration & Architectural Discovery
- Task 1.1: Comprehensive repository audit of `src/`, `sdk/`, `builder/`, `scripts/`, `Cargo.toml`.
- Task 1.2: Map out existing intent engine, AI bridge, graphics/VGA rendering, persistence, and app structures.
- Task 1.3: Finalize `PROJECT.md` interface contracts and milestone breakdown.

### Phase 2: R1 - Rich Component Library & Zero-Lag Component Wiring Engine
- Task 2.1: Native component library (widgets, physics engines, game state modules like Minesweeper & Snake).
- Task 2.2: Intent-to-wiring protocol translator in ArcAI bridge.
- Task 2.3: Zero-lag instant app rendering pipeline.

### Phase 3: R2 - OS-Level Preference Memory & Persistence System
- Task 3.1: OS-level persistence storage driver / host bridge interface.
- Task 3.2: Auto-boot preference loading and layout restoration.
- Task 3.3: Theme/layout serialization and deserialization.

### Phase 4: R3 - Intermittent User Checkup System
- Task 4.1: Proactive checkup trigger & conversational prompt engine.
- Task 4.2: User feedback profiling & preference refinement integration.

### Phase 5: R4 - Dynamic Resource Creation & Persistent Caching
- Task 5.1: Fallback generation for novel components/resources.
- Task 5.2: Persistence storage and reuse pipeline for dynamically generated resources.

### Phase 6: E2E Verification & Forensic Audit
- Task 6.1: Comprehensive E2E test suite validation (Tiers 1-4).
- Task 6.2: Adversarial coverage hardening (Tier 5).
- Task 6.3: Forensic integrity audit verification.
