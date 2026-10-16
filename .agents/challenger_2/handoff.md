# Handoff Report — Challenger 2 (Adversarial Verification)

## 1. Observation

### Build Verification Results
- **Command 1**: `cargo check --target x86_64-arctos.json`
  - **Result**: PASSED (Finished dev profile target in 0.18s)
  - **Warnings**: 7 warnings (unused variable `details` in `src/shell.rs:517:65`, unused functions `find_file_by_semantic_hash`, `find_filename_by_semantic_hash`, `run_smoke_test`, `draw_string`, `draw_line`, constant `SERIAL_TIMEOUT_TICKS`).
- **Command 2**: `cargo check --package arctos_sdk --target x86_64-arctos.json`
  - **Result**: PASSED (Finished dev profile target in 0.11s)
  - **Warnings**: 3 unknown lint warnings (`#[allow(suspicious_runtime_symbol_definitions)]` in `sdk/src/lib.rs`).
- **Command 3**: `cargo check --package arctos_builder --target x86_64-pc-windows-msvc`
  - **Result**: PASSED (Finished dev profile target in 0.10s)
  - **Warnings**: 1 unused `mut` warning (`builder/src/main.rs:139:9`).

---

### Adversarial Findings by Operational Area

#### Area A: Protocol Parsing & VM Bytecode Wire Robustness
- **Panic Bug in `parse_vm_op` (`src/apps/mod.rs:361-364`)**:
  ```rust
  "mod_var" if fields.len() >= 3 => Some(VmOp::ModVar {
      name: String::from(fields[1].trim()),
      modulus: fields[3].trim().parse().unwrap_or(1),
  }),
  ```
  - **Observed**: Wire instruction `OP:mod_var,name,modulus` consists of 3 fields (`fields[0]` = `"mod_var"`, `fields[1]` = name, `fields[2]` = modulus). The condition checks `fields.len() >= 3` but the code accesses `fields[3]`.
  - **Impact**: Any valid 3-field `mod_var` instruction causes a kernel panic due to array index out of bounds (`len is 3, index is 3`). If `fields.len() > 3`, `fields[2]` is ignored and `fields[3]` is read.

#### Area B: AIVFS Inode Layout Mismatch Between Kernel & Host Builder
- **Kernel Layout (`src/fs/structures.rs` & `src/fs/mod.rs`)**:
  - Inode size: 64 bytes
  - `name`: 32 bytes (`[0..32]`)
  - `size`: 4 bytes (`[32..36]`)
  - `created_at`: 8 bytes (`[36..44]`)
  - `data_start_sector`: 4 bytes (`[44..48]`)
  - `sector_count`: 2 bytes (`[48..50]`)
  - `flags`: 1 byte (`[50]`)
  - `semantic_tag_hash`: 4 bytes (`[51..55]`)
  - `embedding_sector`: 4 bytes (`[55..59]`)
  - `_reserved`: 5 bytes (`[59..64]`)
- **Host Builder Layout (`builder/src/main.rs:16-48`)**:
  - Inode size: 64 bytes
  - `name`: 16 bytes (`[0..16]`)
  - `size`: 4 bytes (`[16..20]`)
  - `created_at`: 8 bytes (`[20..28]`)
  - `data_start_sector`: 4 bytes (`[28..32]`)
  - `sector_count`: 2 bytes (`[32..34]`)
  - `flags`: 1 byte (`[34]`)
- **Impact**: Inodes serialized by `arctos_builder` shift all numerical fields left by 16 bytes. When `arctos_os` reads a disk image produced by `arctos_builder`, it reads `flags` from byte 50 (which is 0 / `FLAG_UNUSED`), rendering files written by `arctos_builder` completely invisible to the kernel. Furthermore, `arctos_builder` lacks support for `semantic_tag_hash` (FNV-1a 32-bit hash).

#### Area C: Component Wiring, Dynamic Resource Creation, & Interactive Calculator
- **Negative Subtraction Expression Flaw (`src/apps/mod.rs:828-834`)**:
  - `eval_arithmetic(expr)` evaluates expressions like `"-5-3"`.
  - Calling `expr.split('-')` on `"-5-3"` returns `["", "5", "3"]` (length 3). The parser checks `if parts.len() == 2`, which evaluates to false.
  - Subtractions starting with a negative sign fail to compute and return the original unparsed string.
- **Unbounded Dynamic Memory Allocation Risk (`src/intent.rs:221-229`)**:
  - `Intent::AllocMemory(bytes)` allocates a heap vector and calls `core::mem::forget(chunk)`.
  - The kernel heap size is fixed at 1 MiB (`HEAP_SIZE = 1024 * 1024` in `src/allocator.rs`). Allocation requests or cumulative requests exceeding 1 MiB trigger an unrecoverable kernel heap allocation panic (OOM).

#### Area D: Preference Memory & Checkup Engine Persistence
- **Checkup Prompt State Invariant (`src/user_profile.rs:271-284` & `329-335`)**:
  - `CheckupEngine::record_intent_action_and_check()` returns the `FirstBoot` prompt on every intent action as long as `is_first_boot` is `true`.
  - `is_first_boot` is only set to `false` when a checkup response starting with `"role:"` is received via GUI option click. Pure terminal/shell intent commands leave `is_first_boot = true` persistently in `user_profile.cfg`.
- **Hotspot Wiring & Surface Switching (`src/intent.rs` & `src/bga.rs`)**:
  - Hotspot click hit-testing in `bga.rs:777-800` correctly resolves corner hotspot bounds and dispatches intents (`DevWorkspace`, `AITensorWorkspace`). Snapshotting `ACTIVE_APP.lock().clone()` prior to calling `handle_button_click` prevents re-entrant deadlock.

---

## 2. Logic Chain

1. **Protocol Parsing Panic Reasoning**:
   - `parse_vm_op` takes an opcode segment `s` and calls `s.splitn(10, ',')`.
   - For `OP:mod_var,x,10`, `fields` is `["mod_var", "x", "10"]` (`len = 3`).
   - Line 361 evaluates `fields.len() >= 3` as `true`.
   - Line 363 accesses `fields[3]`, which throws `index out of bounds: the len is 3 but the index is 3` -> Panic.

2. **AIVFS Disk Incompatibility Reasoning**:
   - `builder/src/main.rs:17-25` parses inodes with a 16-byte `name` array at offset 0, placing `size` at offset 16 and `flags` at offset 34.
   - `src/fs/mod.rs:90-101` parses inodes with a 32-byte `name` array at offset 0, placing `size` at offset 32 and `flags` at offset 50.
   - Byte 50 of an inode written by `arctos_builder` contains 0 (padding).
   - In `src/fs/mod.rs:205`, `node.flags != FLAG_FILE` (1) skips the inode as unused.
   - Therefore, disk images created by `arctos_builder` fail to present pre-loaded files to `arctos_os`.

3. **Calculator Parsing Reasoning**:
   - `eval_arithmetic("-5-3")` checks `expr.contains('-')` first.
   - `"-5-3".split('-')` yields 3 items: `""`, `"5"`, `"3"`.
   - `parts.len() == 2` evaluates to false.
   - Function falls through and returns `"-5-3"`, failing to evaluate `-8`.

4. **Heap Exhaustion Reasoning**:
   - `HEAP_SIZE` in `src/allocator.rs` is `1024 * 1024` bytes (1 MiB).
   - `Intent::AllocMemory` forgets the allocated memory block (`core::mem::forget`).
   - Any single `alloc 2048kb` or cumulative allocations totaling > 1 MiB exhaust `ALLOCATOR`, leading to kernel panic on next allocation.

---

## 3. Caveats

- **Hardware Testing**: Verification was performed via host compilation (`cargo check`) and static code tracing. Bare-metal hardware / QEMU execution was not executed in this subagent pass.
- **Skill Instructions**: No external domain skills were loaded.

---

## 4. Conclusion

- **Build Quality**: All three requested targets compile cleanly (`cargo check --target x86_64-arctos.json`, `cargo check --package arctos_sdk --target x86_64-arctos.json`, `cargo check --package arctos_builder --target x86_64-pc-windows-msvc`).
- **High-Severity Bugs Identified**:
  1. **VM Opcode Parser Out-of-Bounds Panic**: `parse_vm_op` (`src/apps/mod.rs:361`) panics on valid `mod_var` bytecode.
  2. **AIVFS Inode Layout Mismatch**: `builder/src/main.rs` and `src/fs/mod.rs` have conflicting 64-byte inode field offsets, corrupting disk image file reads between builder and kernel.
- **Medium-Severity Bugs Identified**:
  1. `eval_arithmetic` fails on negative number subtraction strings.
  2. `Intent::AllocMemory` lacks heap bounds guard checking prior to `alloc::vec!`.

---

## 5. Verification Method

To verify these findings:

1. **Build Checks**:
   ```powershell
   cargo check --target x86_64-arctos.json
   cargo check --package arctos_sdk --target x86_64-arctos.json
   cargo check --package arctos_builder --target x86_64-pc-windows-msvc
   ```

2. **VM Opcode Out-of-Bounds Inspection**:
   Inspect line 361 of `src/apps/mod.rs`:
   ```rust
   "mod_var" if fields.len() >= 3 => Some(VmOp::ModVar {
       name: String::from(fields[1].trim()),
       modulus: fields[3].trim().parse().unwrap_or(1),
   }),
   ```
   Note `fields.len() >= 3` vs `fields[3]`.

3. **AIVFS Inode Layout Inspection**:
   Compare `parse_inode` in `builder/src/main.rs:16-35` against `read_inode_table` in `src/fs/mod.rs:90-112`.
   Observe `name` length of 16 vs 32 and `flags` byte offset of 34 vs 50.
