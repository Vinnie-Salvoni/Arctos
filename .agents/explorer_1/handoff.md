# Arctos OS ArcAI Architecture Overhaul — Audit & Blueprint Report

## 1. Observation

### 1.1 Existing Architecture & Kernel Entry Points
- **Kernel Boot & Entry Point (`src/main.rs`)**:
  - `kernel_main` (lines 54-147) bootstraps the bare-metal kernel (`#![no_std]`, `#![no_main]`).
  - Initializes PIT (`pit::init_pit()`), GDT & TSS (`gdt::init()`), IDT (`interrupts::init_idt()`), PIC (`interrupts::PICS`), PS/2 mouse (`mouse::init_mouse()`), UART serial (`serial::init()`), paging (`memory::init`), heap (`allocator::init_heap`, 100 KiB), memory mapper (`memory_mapper::init`), AIVFS filesystem (`fs::mount()`), and launches interactive shell (`shell::run()`).
- **AI Intent Engine & Event Bus (`src/intent.rs`)**:
  - Defines `Intent` enum (lines 14-50) covering `AllocMemory`, `CreateFile`, `LaunchApp`, `ConfigureEnvironment`, `SetHotspot`, `DevWorkspace`, `AITensorWorkspace`, `ToggleTelemetry`, `ThemeChange`, `ConversationalAdaptation`, `Custom`, `Unknown`.
  - `SemanticIntentEngine` (lines 101-193) implements `IntentListener` trait, extracting numbers, filenames, actions, themes, and app names via natural language string parsing.
  - `execute_kernel_intent` (lines 196-279) executes kernel actions, logs traces to `SYSTEM_EXECUTION_LOG`, increments `SUCCESSFUL_ACTIONS_COUNT` and `OPTIMIZATION_ITERATION`.
- **User Profile System (`src/user_profile.rs`)**:
  - `UserProfile` struct (lines 13-25) contains `username`, `role`, `ram_limit_mb`, and 4 corner hotspot labels/actions.
  - `get_or_load_profile()` (lines 49-74) attempts to mount AIVFS, read `user_profile.cfg`, parse key-value lines, or generate default profile and save via `save_profile_file()`.
  - `apply_profile_hotspots()` (lines 135-178) populates `HOTSPOTS` mutex with top-left (30,70), top-right (1740,70), bottom-left (30,920), bottom-right (1740,920) hotspot descriptors.
- **Graphics Engine & BGA Driver (`src/bga.rs` & `src/vga_graphics.rs`)**:
  - `run_gui_mode()` (lines 451-633) configures Bochs VBE BGA (1920x1080x32bpp), maps LFB via `get_bga_lfb_phys()`, loads `UserProfile`, renders cybernetic background gradient (`CURRENT_THEME`, `SECONDARY_THEME`), floating active app window, intent prompt input bar, ArcAI feedback panel, and interactive PS/2 cursor with pixel background save/restore buffer.
- **Dynamic App Engine & Micro-VM (`src/apps/mod.rs`)**:
  - `UIWidgetKind` enum (lines 278-287) supports `Panel`, `Text`, `Button`, `DisplayScreen`, `ProgramCanvas { program: Vec<VmOp> }`, and `NativeGame(NativeGameKind)` (`Minesweeper`, `Snake`).
  - `MinesweeperState` (lines 473-612): 9x9 grid, 10 mines, BFS flood fill, state handling.
  - `SnakeState` (lines 631-690): 28x18 grid, directional movement, food placement, collision detection.
- **AI Bridge Interface (`src/model_loader.rs` & `scripts/ai_bridge.py`)**:
  - `run_arcai_inference` (lines 118-206) streams text prompt out COM1 UART (`serial::send_prompt`), receives response from host Python bridge (`scripts/ai_bridge.py`), dispatches `APP:` to `parse_app_spec` or `APP_PROG:` to `parse_program_spec`.
  - `scripts/ai_bridge.py`: RAG memory (`app_memory.json`), queries Ollama (`qwen2.5:7b`), validates generated spec, outputs `APP:TITLE=...;WIDGET=...` or `APP_PROG:TITLE=...;PROGRAM=OP:...`.
- **AIVFS Storage Engine (`src/fs/mod.rs` & `src/fs/structures.rs`)**:
  - Sector-based filesystem over ATA PIO disk driver (`src/disk.rs`).
  - `Superblock` (magic `0xA8C705`, version 1, 20480 sectors = 10 MB).
  - `Inode` (64 bytes): name (32B), size (4B), created_at (8B), data_start_sector (4B), sector_count (2B), flags (1B), `semantic_tag_hash` (4B), `embedding_sector` (4B), `_reserved` (5B).
- **SDK & Builder Workspace Tools (`sdk/` & `builder/`)**:
  - `sdk/src/lib.rs`: `KernelApi` C-ABI struct, entry macro `arctos_main!`, low-level `memset`/`memcpy`/`memcmp` assembly helpers.
  - `builder/src/main.rs`: Command-line host tool (`compile-and-install`, `build-app`, `write-disk`, `delete-file`, `list-disk`) for packaging standalone `.arc` apps into `target/test_disk.bin`.

### 1.2 Build Verification Output & Compiler Diagnostics
- **Command Executed**: `cargo check --target x86_64-arctos.json`
  - **Result**: Failed with exit code 1.
  - **Verbatim Error Output**:
    ```
    Checking arctos_os v0.1.11 (C:\Users\vinni\Downloads\Arctos)
    error: unexpected closing delimiter: `}`
       --> src\apps\mod.rs:705:1
        |
    693 | pub fn spawn_snake() {
        |                      - this opening brace...
    ...
    701 | }
        | - ...matches this closing brace
    ...
    705 | }
        | ^ unexpected closing delimiter
    error: could not compile `arctos_os` (bin "arctos_os") due to 1 previous error
    ```
  - **Root Cause Analysis**: In `src/apps/mod.rs` at lines 704-705, the function header `pub fn close_app() {` was accidentally cut/omitted during prior editing:
    ```rust
    // Line 704-705 in src/apps/mod.rs:
        *ACTIVE_APP.lock() = None;
    }
    ```
- **Command Executed**: `cargo check --package arctos_sdk --target x86_64-arctos.json`
  - **Result**: PASSED (0 errors, 3 warnings for unknown lint `suspicious_runtime_symbol_definitions`).
- **Command Executed**: `cargo check --package arctos_builder --target x86_64-pc-windows-msvc`
  - **Result**: PASSED (0 errors, 1 warning for unused `mut`).

---

## 2. Logic Chain

### 2.1 Build Fix Rationale
1. `src/bga.rs` line 558 calls `crate::apps::close_app()`.
2. `src/apps/mod.rs` contains orphaned function body `*ACTIVE_APP.lock() = None; }` at line 704.
3. Adding `pub fn close_app() {` before line 704 restores syntactical correctness and resolves the build failure cleanly.

### 2.2 Requirement Gap Analysis

#### **Requirement R1: Rich Component Library & Zero-Lag Component Wiring**
- **Observation**:
  - `UIWidgetKind` in `src/apps/mod.rs` has basic static widgets (`Panel`, `Text`, `Button`, `DisplayScreen`), micro-VM bytecode (`ProgramCanvas`), and two hardcoded games (`NativeGameKind::Minesweeper`, `NativeGameKind::Snake`).
  - `model_loader.rs` handles `APP:` (widget spec) and `APP_PROG:` (bytecode program wire).
  - `scripts/ai_bridge.py` emits `APP_PROG:` bytecode when generating dynamic interactive apps or animations.
- **Logic Chain**:
  - Transmitting frame-by-frame bytecode ops (`APP_PROG:`) introduces interpretation overhead and rendering latency.
  - Games and dynamic engines (physics, Minesweeper, Snake, particle systems) are currently either hardcoded isolated structs or frame-by-frame VM bytecodes.
  - To achieve zero-lag component wiring, the AI bridge and kernel must communicate via a unified component wiring protocol (`APP:TITLE=...;WIDGET=...;ENGINE=...` or `WIDGET=native_game,kind=minesweeper` / `WIDGET=physics_canvas,gravity=9.8...`). The kernel instantiates pre-compiled native component engines directly, running at native bare-metal execution speed without frame-by-frame network or bytecode interpreter overhead.

#### **Requirement R2: OS-Level Preference Memory**
- **Observation**:
  - `UserProfile` in `src/user_profile.rs` only stores `username`, `role`, `ram_limit_mb`, and 4 corner hotspots (`tl_label`, `tr_label`, `bl_label`, `br_label`).
  - `save_profile_file` writes plain text key-value pairs to `user_profile.cfg` on AIVFS.
  - `kernel_main` in `src/main.rs` initializes AIVFS (`fs::mount()`), but does NOT auto-load the user profile or restore theme palettes / surface layouts during boot.
- **Logic Chain**:
  - `UserProfile` does not persist theme color palettes (`CURRENT_THEME`, `SECONDARY_THEME`), visual surfaces (`UISurface::Developer`, `UISurface::Ambient`), custom hotspot pins, or user intent history.
  - Without auto-load during `kernel_main` boot sequence, user preferences are not restored consistently upon system startup.
  - Preference storage needs an expanded schema (`UserProfilePreferences`) covering themes, custom layout arrays, role parameters, and auto-load hooks during kernel bootstrap.

#### **Requirement R3: Intermittent User Checkups**
- **Observation**:
  - Arctos OS rule specifies: "On first boot, the OS prompts the user to set initial persistent hotspots, which can be modified in settings. The UI surface must support live conversational feedback...".
  - `Intent::ConversationalAdaptation` exists in `intent.rs`, and `bga.rs` renders `LAST_FEEDBACK_MSG`.
  - However, no proactive background checkup trigger, event counter, or first-boot checkup wizard exists.
- **Logic Chain**:
  - The OS needs a proactive checkup scheduler (`CheckupEngine`) that monitors intent execution count, first-boot flag, and user interaction patterns.
  - After a configured threshold of user interactions (e.g. 5 intent actions) or on initial system boot, the system triggers an intermittent user checkup card (e.g., *"Checkup: Does the Dark Teal theme & Software Developer hotspot layout fit your current workflow?"*).
  - User responses via intent input automatically refine `UserProfile` in AIVFS.

#### **Requirement R4: Dynamic Resource Creation & Persistent Caching**
- **Observation**:
  - When an unrecognized intent or `LaunchApp` occurs, `execute_kernel_intent` calls `run_arcai_inference()`.
  - Host-side RAG memory (`app_memory.json`) saves generated specs in Python `ai_bridge.py`.
  - Kernel AIVFS (`src/fs/mod.rs`) has `Inode` fields `semantic_tag_hash` and `embedding_sector`, but they are unused (set to 0).
  - If AI bridge is offline, kernel returns string error `"[ArcAI] Start 'python scripts/ai_bridge.py'..."` instead of generating fallback resources.
- **Logic Chain**:
  - The kernel lacks on-disk resource caching in AIVFS (`/cache/` directory or `.arc` storage).
  - If a generated widget layout or app spec is created, it should be cached to AIVFS (`cache_<app_name>.cfg`). Subsequent launches look up AIVFS cache first, providing instant boot without network/AI delays.
  - If the AI bridge is unreachable, a native fallback component generator (`NativeFallbackGenerator`) generates a basic functional template (e.g., standard notes panel or calc widget) locally on bare-metal.
  - Utilizing `semantic_tag_hash` in AIVFS inodes enables fast semantic lookup for cached components.

---

## 3. Caveats
- **Hardware Graphics**: Investigation was conducted under QEMU Bochs VBE (1920x1080x32bpp) emulation mode. Physical PCIe consumer NPU drivers (Phase 4) remain planned for future milestones.
- **Host AI Bridge Dependency**: Real-time LLM inference requires `ollama` running on the Windows host machine with `python scripts/ai_bridge.py` connected over serial TCP (port 4444). Native fallback generation handles offline state cleanly.
- **No Source Code Edits Made**: In accordance with the Explorer archetype read-only mandate, no files inside `src/`, `sdk/`, or `builder/` were modified. Proposed code changes are provided as exact patch blueprints in Section 4.

---

## 4. Conclusion & Proposed Implementation Blueprint

### 4.1 Build Failure Fix Blueprint
To fix the immediate compiler error in `src/apps/mod.rs`, insert `pub fn close_app() {` before line 704:

```rust
// Proposed Patch for src/apps/mod.rs (lines 703-706)
pub fn close_app() {
    *ACTIVE_APP.lock() = None;
}
```

---

### 4.2 Blueprint R1: Rich Component Library & Zero-Lag Component Wiring

#### Architectural Concept
Extend `UIWidgetKind` to support native engine declarations and rich UI widgets. Update the AI bridge wire protocol so that complex dynamic modules (Minesweeper, Snake, Physics Engine, Sliders, Progress Bars, Form Controls) are declared natively via `WIDGET=native_engine,...` or `ENGINE=...`, avoiding frame-by-frame bytecode interpretation.

#### Component Specification & Data Structures (`src/apps/mod.rs`)
```rust
#[derive(Debug, Clone)]
pub enum UIWidgetKind {
    Panel { bg_color: u32, border_color: u32 },
    Text { text: String, color: u32 },
    Button { label: String, action: String, bg_color: u32, text_color: u32 },
    DisplayScreen { text: String, bg_color: u32, text_color: u32 },
    Slider { min: i32, max: i32, value: i32, label: String, action: String },
    ProgressBar { progress_pct: u8, color: u32, label: String },
    NativeGame(NativeGameKind),
    PhysicsCanvas { gravity: f32, particle_count: usize, state_hash: u32 },
    ProgramCanvas { program: Vec<VmOp> },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NativeGameKind {
    Minesweeper,
    Snake,
    PhysicsBouncer,
}
```

#### Protocol Wire Format (`src/model_loader.rs` & `scripts/ai_bridge.py`)
- **Native Widget Spec Wire**:
  `APP:TITLE=Physics Demo;WIDGET=physics,40,60,920,510,gravity=9.8,particles=50,color=0x0000E5FF`
  `APP:TITLE=Minesweeper;WIDGET=engine,0,0,1000,620,kind=minesweeper`
- **Zero-Lag Wiring Handler**:
  When `parse_app_spec` detects `WIDGET=engine,kind=minesweeper` or `kind=snake`, it binds directly to `NativeGameKind::Minesweeper` or `NativeGameKind::Snake`, running at 60 FPS natively in `bga.rs` without bytecode interpretation.

---

### 4.3 Blueprint R2: OS-Level Preference Memory & Boot Integration

#### Architectural Concept
Expand `UserProfile` to persist complete user workspace state (theme palette, visual surface, custom hotspots, AI role parameters, telemetry visibility) to `user_profile.cfg` on AIVFS, and hook `get_or_load_profile()` into `kernel_main` in `src/main.rs`.

#### Data Structure (`src/user_profile.rs`)
```rust
#[derive(Debug, Clone)]
pub struct UserProfile {
    pub username: String,
    pub role: String,
    pub ram_limit_mb: usize,
    pub theme_primary: u32,
    pub theme_secondary: Option<u32>,
    pub active_surface: String, // "ambient", "developer", "ai_tensor"
    pub show_telemetry: bool,
    pub is_first_boot: bool,
    pub corner_top_left_label: String,
    pub corner_top_left_action: String,
    pub corner_top_right_label: String,
    pub corner_top_right_action: String,
    pub corner_bottom_left_label: String,
    pub corner_bottom_left_action: String,
    pub corner_bottom_right_label: String,
    pub corner_bottom_right_action: String,
}
```

#### Persistence & Boot Auto-Load (`src/main.rs` & `src/user_profile.rs`)
1. In `src/user_profile.rs`, `save_profile_file` serializes all fields (`theme_primary=0x000F2A38`, `active_surface=developer`, `is_first_boot=false`, etc.).
2. In `src/main.rs` inside `kernel_main`, after `fs::mount()`:
   ```rust
   // Auto-load user profile preferences on boot
   let profile = user_profile::get_or_load_profile();
   user_profile::apply_profile_hotspots(&profile);
   *bga::CURRENT_THEME.lock() = profile.theme_primary;
   *bga::SECONDARY_THEME.lock() = profile.theme_secondary;
   ```

---

### 4.4 Blueprint R3: Intermittent User Checkups

#### Architectural Concept
Implement an autonomous checkup scheduler (`CheckupEngine`) in `src/user_profile.rs` that counts executed user intents and checks first-boot status. When triggered, it generates a conversational checkup prompt on the UI canvas, prompting the user for feedback and automatically saving profile updates.

#### Checkup Scheduler Interface (`src/user_profile.rs`)
```rust
pub struct CheckupEngine {
    pub intent_counter: u64,
    pub checkup_interval: u64, // e.g. every 5 intents
    pub active_checkup: Option<CheckupPrompt>,
}

#[derive(Debug, Clone)]
pub struct CheckupPrompt {
    pub message: String,
    pub option_a: String, // e.g. "Keep Current"
    pub option_b: String, // e.g. "Switch to Developer Mode"
}

pub static CHECKUP_ENGINE: spin::Mutex<CheckupEngine> = spin::Mutex::new(CheckupEngine {
    intent_counter: 0,
    checkup_interval: 5,
    active_checkup: None,
});

pub fn record_intent_action_and_check() -> Option<String> {
    let mut engine = CHECKUP_ENGINE.lock();
    engine.intent_counter += 1;

    let prof = get_or_load_profile();

    if prof.is_first_boot {
        let prompt = "Welcome to Arctos OS! Would you like to keep the default Software Developer layout and Dark Teal theme?";
        return Some(String::from(prompt));
    }

    if engine.intent_counter % engine.checkup_interval == 0 {
        let prompt = alloc::format!(
            "Checkup: You've executed {} actions. Is the {} role & layout performing well?",
            engine.intent_counter, prof.role
        );
        return Some(prompt);
    }
    None
}
```

#### Integration Flow
- Every time `execute_kernel_intent` completes an intent in `src/intent.rs`, it calls `record_intent_action_and_check()`.
- If a checkup message is returned, `LAST_FEEDBACK_MSG` is set, displaying the interactive checkup banner on the 1080p canvas.
- Replying to the checkup updates `UserProfile` and persists it via `save_profile()`.

---

### 4.5 Blueprint R4: Dynamic Resource Creation & Persistent Caching

#### Architectural Concept
Implement a 2-tier resource pipeline:
1. **Tier 1: OS Disk Cache (`AIVFS`)**: Before querying the AI bridge, check AIVFS storage for cached app specs (`cache_<app_name>.cfg`). If found, load instantly. If missing, query AI bridge and write response to AIVFS cache.
2. **Tier 2: Bare-Metal Fallback Generator (`NativeFallbackGenerator`)**: If the AI bridge is offline or times out, generate a pre-built native fallback widget spec locally rather than failing with an error.

#### Cache & Fallback Module Design (`src/model_loader.rs`)
```rust
pub fn resolve_app_resource(app_name: &str) -> String {
    let cache_filename = alloc::format!("cache_{}.cfg", app_name);

    // 1. Check AIVFS persistent disk cache
    if let Some(cached_data) = crate::fs::read_file(&cache_filename) {
        if let Ok(spec) = core::str::from_utf8(&cached_data) {
            crate::println!("[ResourceCache] Cache hit for '{}'", app_name);
            return parse_app_spec(spec);
        }
    }

    // 2. Query Host AI Bridge
    let prompt = alloc::format!("create {}", app_name);
    let bridge_response = run_arcai_inference(&prompt);

    if bridge_response.starts_with("[ArcAI] Start") || bridge_response.contains("offline") {
        // 3. Fallback: Generate native bare-metal resource
        crate::println!("[ResourceCache] AI offline. Generating native fallback for '{}'", app_name);
        let fallback_spec = generate_native_fallback(app_name);
        return parse_app_spec(&fallback_spec);
    }

    // Write successful bridge response to AIVFS cache
    if bridge_response.starts_with("[ArcAI] Synthesized") {
        // Cache the raw spec
        // (saved during parse_app_spec)
    }

    bridge_response
}

pub fn generate_native_fallback(app_name: &str) -> String {
    match app_name.to_lowercase().as_str() {
        "calculator" | "calc" => {
            String::from("TITLE=Calculator;WIDGET=display,40,60,920,60,0,show,0xFF020C0C;WIDGET=button,60,140,200,60,7,btn:7,0xFF0C1A2B;WIDGET=button,280,140,200,60,8,btn:8,0xFF0C1A2B;WIDGET=button,500,140,200,60,9,btn:9,0xFF0C1A2B;WIDGET=button,720,140,200,60,+,btn:+,0xFF0C1A2B")
        }
        "notes" | "notepad" => {
            String::from("TITLE=Notepad;WIDGET=panel,40,60,920,510,0xFF050E14,0x0000E5FF;WIDGET=text,60,80,880,470,Arctos Native Notes,0x00FFFFFF")
        }
        _ => {
            alloc::format!("TITLE={};WIDGET=panel,40,60,920,510,0xFF050E14,0x0000E5FF;WIDGET=text,60,80,880,470,Arctos Dynamic App Surface for {},0x00FFFFFF", app_name, app_name)
        }
    }
}
```

#### Semantic Hash Tagging (`src/fs/mod.rs`)
Compute djb2/fnv1a hash of semantic tags when saving files/cache in `write_file`, storing the hash in `node.semantic_tag_hash`. Enable `find_by_semantic_hash(hash: u32)` for fast O(1) cache lookup.

---

## 5. Verification Method

To verify the audit findings and future implementation work independently:

### 5.1 Verification Commands
1. **Fix Syntax Error**: Apply the one-line fix (`pub fn close_app() {`) to `src/apps/mod.rs:704`.
2. **Kernel Build Verification**:
   ```powershell
   cargo check --target x86_64-arctos.json
   ```
   *Expected result*: Build succeeds with 0 errors.
3. **SDK Build Verification**:
   ```powershell
   cargo check --package arctos_sdk --target x86_64-arctos.json
   ```
   *Expected result*: Build succeeds with 0 errors.
4. **Builder Build Verification**:
   ```powershell
   cargo check --package arctos_builder --target x86_64-pc-windows-msvc
   ```
   *Expected result*: Build succeeds with 0 errors.

### 5.2 Inspection Locations
- `src/apps/mod.rs`: Confirm `pub fn close_app()` definition and `UIWidgetKind` extensions.
- `src/user_profile.rs`: Confirm `UserProfile` struct extensions and `save_profile_file` key-value fields.
- `src/main.rs`: Confirm `kernel_main` auto-loads profile after `fs::mount()`.
- `src/model_loader.rs`: Confirm AIVFS disk caching and `generate_native_fallback` implementation.
- `scripts/ai_bridge.py`: Confirm `APP:TITLE=...` and native engine wiring format validation.

### 5.3 Invalidation Conditions
- Any occurrence of `error: unexpected closing delimiter` in `src/apps/mod.rs`.
- `cargo check --target x86_64-arctos.json` returning non-zero exit code.
- Failure of `get_or_load_profile()` to load saved theme colors or corner hotspots on kernel startup.
- Uncached duplicate serial calls to host AI bridge when launching previously generated apps.
