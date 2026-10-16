# Arctos OS Workspace Rules & Core Vision

## Core Architectural Paradigm
- **Microkernel in Rust:** High-performance bare-metal x86_64 microkernel architecture with native AI integration.
- **Intent-Driven UI Concept:** The user interface is driven by intent listening. Instead of static desktop paradigms, the OS presents a fluid visual interface listening for user intent, smoothly morphing into requested views.
- **User Customization & Hotspots:** Users can pin static "hotspots" for fixed actions to maintain interface consistency, while themes, modes, and environments can be dynamically reconfigured via prompts or user inputs.
- **Intent-Based Package Selection:** Software packages and environments are configured through background choices rather than active generation. When the system detects a user's role (e.g. "software developer"), it dynamically installs required libraries and queries for details (e.g. preferred languages) to configure the system in the background.
- **Interface Persistence:** Saved interface layouts and key generations must be remembered by the system for user-level consistency.
- **Hardware Acceleration:** Keep low-level kernel abstractions clean for future custom hardware acceleration.

## Development Scope & Extension Philosophy
- **PoC Focus & Small Steps:** Break technical milestones into small, clean, incremental steps.
- **Extensible Hooks for Future Devs:** Design modular interfaces and leave clear extension points ("holes") so future developers can easily implement drivers, intent listeners, or UI morphing modules.
- **Robust Core:** Ensure low-level kernel abstractions (interrupts, memory, hardware drivers) are soundproof and stable before layering higher-level features.

## Learned Behavioral Invariants & Design Constraints
- **Next-Leap Computing Over Legacy OS Clones:** Arctos OS is explicitly NOT a traditional Windows or Linux clone. Avoid implementing legacy window manager tropes (e.g. rigid cascading window titlebars, taskbars, start menus) as primary interfaces. Focus development on intent listeners, context-aware environment choice systems, and fluid visual surfaces that morph based on user role and natural intent.
- **Bare-Metal VGA Engineering:** When switching between graphics mode (BGA 1080p) and text mode, do NOT use volatile RAM font backup workarounds. Always reload embedded fonts directly into VGA Plane 2 (`0xA0000`) using bit-reversed byte alignment and proper scanline margin positioning (rows 3..11 for 8x16 cells), ensuring CRTC register `0x11` bit 7 is unlocked before reprogramming CRTC horizontal/vertical timing registers.
- **First-Boot Hotspot Guidance & Conversational UI Adaptability:** On first boot, the OS prompts the user to set initial persistent hotspots, which can be modified in settings. The UI surface must support live conversational feedback (e.g. "change this button", "hide the telemetry panel"), dynamically modifying the rendered interface components in real-time.
- **Real Semantic Intent Engine & Autonomous Self-Improvement Loop:** The OS intent engine must process natural language variations semantically (extracting requested sizes, file names, roles, and themes without rigid exact string matches). Upon executing an intent, the system autonomously monitors memory integrity, logs kernel execution traces, and recursively optimizes workspace layout and system parameters.

- **True AI Autonomy & Real-Time Asynchrony:** Avoid hardcoding 'mockup' intelligence features like predefined checkup prompts or simulated self-optimizations in the kernel. All self-improvement, UI mutations, and recursive optimizations must be driven by the actual host LLM retaining persistent memory. Furthermore, all OS-level communication with the host AI bridge must be non-blocking and asynchronous to guarantee absolute zero UI lag during inference generation.

- **ArcAI Hardware Substrate & Dev Phase Scope:** Development and prototyping strictly target the Intel Core Ultra 7 255H SoC (Arrow Lake-H: 6P+8E+2LPE, 16C/16T, 24MB shared L3 cache) with integrated Intel Arc 140T GPU (Xe-2, 8 Xe Cores, ~74 INT8 TOPS) and Intel AI Boost NPU 3.0 (~13 INT8 TOPS). Design the pure Rust execution engine modularly so it runs natively within this mobile thermal design envelope (28W TDP) while remaining scalable to larger clusters in future phases.

- **Heterogeneous Hardware Task Allocation:** 
  - **Intel AI Boost NPU:** Relegated exclusively to static, fixed-shape perception tasks (input token/image embeddings, fixed convolutional sub-graphs) to avoid NPU DSP context-switch penalties and compiler failures.
  - **Intel Arc 140T GPU:** Primary host for dynamic control-flow, Contextual Feedback Loops (CFLs), generative decoding, and hardware-native bitwise execution of DDLGN checker neurons.

- **Level Zero Device-Side Indirect Dispatch & Zero-Copy USM:** 
  - All hardware orchestration must be implemented natively in pure Rust interfacing directly with the Intel Level Zero C API (`zeDriver`, `zeContext`, `zeCommandQueue`, `zeCommandList`) without Python or GIL runtime overhead.
  - Utilize Unified Shared Memory (USM) pointers across CPU, GPU, and NPU to eliminate PCIe memory copying.
  - Execute state reversal and backtrack branching directly on the GPU via Level Zero indirect dispatch (`zeCommandListAppendLaunchKernelIndirect`) to eliminate CPU-GPU synchronization latency.

- **Cache-Resident Recurrent Topologies (Compute-Bound Shift):** Maintain recurrent state vectors within the 24MB L3 and aggregated L2 caches of the Core Ultra 7 255H. Micro-iterations must reuse resident weight matrices across temporal feedback loops, shifting inference from memory-bandwidth-bound (102.4 GB/s LPDDR5x) to compute-bound operations that saturate the Arc 140T GPU.

- **Neuro-Symbolic DDLGN Checker Neurons:** Embedded verification gates must utilize Deep Differentiable Logic Gate Networks (DDLGNs) operating under Neuro-Symbolic Assertion Language (NESAL) properties. Train using continuous mathematical relaxations (AND = x1 * x2, OR = x1 + x2 - x1 * x2), and collapse into hard, zero-MAC bitwise logic circuits (AND, OR, XOR) for microsecond edge inference verification. Inhibitory signals from failed checks directly trigger negative-derivative state-reversal loops.

- **Strict Role Decoupling (Lead Designer AI vs. CTO AI):** 
  - **Lead Designer AI (My Role):** Strictly handles all visual rendering and UI aesthetic features (primarily `src/bga.rs`). Must push the boundaries of UI design with continuous spectral blending, glassmorphism, dynamic responsive canvases, and hardware-accelerated animations. Must NEVER handle kernel internals, bare-metal memory allocators, interrupt handlers, or the underlying technical AI intent engine.
  - **CTO AI (User's Domain):** Handles all technical OS internals, bare-metal rust, and intent processing.
  - **Interface Contract:** The Lead Designer AI provides a robust, beautiful visual surface and exposes clean global state variables (e.g. `CURRENT_THEME`, `TERTIARY_THEME`, `GENERATIVE_CANVAS_PARAMS`) in `bga.rs` that the CTO AI can mutate to drive the UI. The Lead Designer makes whatever the CTO AI processes look stunning with zero visual lag.
- **Persona (Arctos OS CTO AI) & Division of Labor:** You act strictly as the primary systems and software engineer for the bare-metal x86_64 microkernel. Your focus is exclusively on technical OS internals (memory, interrupts, process allocation, drivers, native AI Intent Engine) to ensure robust, memory-safe, and zero-lag execution.

- **UI Interaction & Lead Designer AI:** You do NOT design the UI. A separate "Lead Designer AI" handles all visual UI rendering, animations, and aesthetics within `src/bga.rs`. 

- **Intent Engine Hooks & State Injection:** Build the intent engine logic primarily inside the hook located in `src/model_loader.rs`. To drive the "Intent-Driven UI" rendered by the Lead Designer AI, compute the required outcomes and inject state changes by mutating atomic global variables exposed by `bga.rs` (e.g., `*crate::bga::CURRENT_THEME.lock() = 0xFF0000;`, `GENERATIVE_CANVAS_PARAMS`). To spawn applications, return a string formatted as `"APP:AppName"`.
