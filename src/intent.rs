// =============================================================================
// Arctos OS — Modular Intent Listener & Event Bus
// =============================================================================
// This module defines the core intent-driven abstractions for Arctos OS.
// It exposes clean traits and extension hooks so future developers
// can easily plug in LLM intent parsers, voice listeners, or custom hardware accelerators.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use spin::Mutex;

/// High-level intents recognized by the Arctos OS core.
#[derive(Debug, Clone, PartialEq)]
pub enum Intent {
    /// Real Kernel Action: Allocate memory bytes on heap
    AllocMemory(usize),

    /// Real Kernel Action: Create/write file on AIVFS storage
    CreateFile { filename: String, content: String },

    /// Launch a standalone application (e.g., "stopwatch", "notepad").
    LaunchApp(String),

    /// Configure user environment/role (e.g., "software developer", "ai engineer").
    ConfigureEnvironment { role: String, details: Option<String> },

    /// Pin a persistent hotspot widget to the UI canvas.
    SetHotspot { x: usize, y: usize, label: String, action: String },

    /// Switch UI workspace to Developer Surface.
    DevWorkspace,

    /// Switch UI workspace to AI Tensor Surface.
    AITensorWorkspace,

    /// Toggle background system telemetry panel on canvas.
    ToggleTelemetry(bool),

    /// Change active desktop UI theme color palette.
    ThemeChange(String),

    /// Conversational dynamic UI adaptation.
    ConversationalAdaptation(String),

    /// Load/restore persistent user profile preferences.
    LoadUserProfile,

    /// Custom/Unrecognized intent for future expansion plugins.
    Custom { domain: String, payload: String },

    /// Raw intent string that could not be parsed into a known category.
    Unknown(String),
}

/// Represents a persistent interactive hotspot anchored on the 1080p canvas.
#[derive(Debug, Clone, PartialEq)]
pub struct Hotspot {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    pub label: String,
    pub action: String,
}

pub static HOTSPOTS: Mutex<Vec<Hotspot>> = Mutex::new(Vec::new());
// System Execution Log
pub static SYSTEM_EXECUTION_LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub fn add_hotspot(x: usize, y: usize, label: &str, action: &str) {
    let mut spots = HOTSPOTS.lock();
    spots.push(Hotspot {
        x,
        y,
        width: 140,
        height: 40,
        label: label.to_string(),
        action: action.to_string(),
    });
}

pub fn find_clicked_hotspot(mouse_x: usize, mouse_y: usize) -> Option<Hotspot> {
    let spots = HOTSPOTS.lock();
    for spot in spots.iter() {
        if mouse_x >= spot.x && mouse_x < spot.x + spot.width &&
           mouse_y >= spot.y && mouse_y < spot.y + spot.height {
            return Some(spot.clone());
        }
    }
    None
}

/// Extensible trait for intent listeners and AI parsers.
pub trait IntentListener: Send + Sync {
    fn parse_intent(&self, raw_input: &str) -> Intent;
}

/// Native Semantic Intent Parser — understands natural language variations,
/// extracts numbers, filenames, actions, themes, and roles semantically.
pub struct SemanticIntentEngine;

impl IntentListener for SemanticIntentEngine {
    fn parse_intent(&self, raw_input: &str) -> Intent {
        let trimmed = raw_input.trim();
        let lower = trimmed.to_lowercase();

        // Direct System Mode Switches
        if lower == "dev" || lower == "developer" {
            return Intent::DevWorkspace;
        } else if lower == "ai" || lower == "tensor" {
            return Intent::AITensorWorkspace;
        }

        // Pass all natural language prompts directly to ArcAI
        Intent::Unknown(trimmed.to_string())
    }
}

/// Executes real kernel actions for parsed intents, logging empirical traces and driving self-improvement.
pub fn execute_kernel_intent(intent: &Intent) -> String {

    let mut logs = SYSTEM_EXECUTION_LOG.lock();

    match intent {
        Intent::AllocMemory(bytes) => {
            let size = *bytes;
            let chunk = alloc::vec![0x5Au8; size].into_boxed_slice();
            let ptr_str = alloc::format!("0x{:p}", chunk.as_ptr());
            let log_entry = alloc::format!("Heap Allocated {} bytes at {}", size, ptr_str);
            logs.push(log_entry.clone());
            core::mem::forget(chunk);
            log_entry
        }
        Intent::CreateFile { filename, content } => {
            let ok = crate::fs::write_file(filename, content.as_bytes());
            let log_entry = if ok {
                alloc::format!("AIVFS Wrote file '{}' ({} bytes)", filename, content.len())
            } else {
                alloc::format!("Failed to write file '{}'", filename)
            };
            logs.push(log_entry.clone());
            log_entry
        }
        Intent::DevWorkspace => {
            let log_entry = alloc::format!("Workspace morphed to Developer Surface.");
            logs.push(log_entry.clone());
            log_entry
        }
        Intent::AITensorWorkspace => {
            let log_entry = alloc::format!("Workspace morphed to AI Tensor Surface.");
            logs.push(log_entry.clone());
            log_entry
        }
        Intent::ThemeChange(theme) => {
            let log_entry = alloc::format!("UI Theme mutated to '{}' palette.", theme);
            logs.push(log_entry.clone());
            log_entry
        }
        Intent::SetHotspot { label, action, .. } => {
            let log_entry = alloc::format!("Hotspot '{}' anchored for action: {}", label, action);
            logs.push(log_entry.clone());
            log_entry
        }
        Intent::LoadUserProfile => {
            let prof = crate::user_profile::get_or_load_profile();
            *crate::bga::CURRENT_THEME.lock() = prof.theme_primary;
            *crate::bga::SECONDARY_THEME.lock() = prof.theme_secondary;
            *crate::bga::SHOW_TELEMETRY.lock() = prof.show_telemetry;
            match prof.active_surface.as_str() {
                "ai" | "tensor" => *crate::bga::CURRENT_SURFACE.lock() = crate::bga::UISurface::AITensorWorkspace,
                _ => *crate::bga::CURRENT_SURFACE.lock() = crate::bga::UISurface::Developer,
            }
            crate::user_profile::apply_profile_hotspots(&prof);
            let log_entry = alloc::format!("[UserProfile] Loaded user '{}' (role: '{}') preferences from user_profile.cfg.", prof.username, prof.role);
            logs.push(log_entry.clone());
            log_entry
        }
        Intent::ConversationalAdaptation(msg) => {
            let log_entry = crate::model_loader::run_arcai_inference(msg);
            logs.push(log_entry.clone());
            log_entry
        }
        Intent::ToggleTelemetry(val) => {
            let log_entry = alloc::format!("Telemetry panel toggled: {}", val);
            logs.push(log_entry.clone());
            log_entry
        }
        Intent::LaunchApp(app) => {
            // Ask the AI bridge to generate a real interactive widget spec.
            // run_arcai_inference will parse APP: responses and call spawn_dynamic_app.
            // If the bridge is offline it falls back gracefully with a text message.
            let prompt = alloc::format!("create {}", app);
            let log_entry = crate::model_loader::run_arcai_inference(&prompt);
            logs.push(log_entry.clone());
            log_entry
        }
        Intent::Unknown(raw) => {
            let log_entry = crate::model_loader::run_arcai_inference(raw);
            logs.push(log_entry.clone());
            log_entry
        }
        _ => {
            let log_entry = alloc::format!("Registered intent payload.");
            logs.push(log_entry.clone());
            log_entry
        }
    }
}

/// Global Asynchronous Intent Event Bus.
pub static INTENT_BUS: Mutex<Vec<Intent>> = Mutex::new(Vec::new());

pub fn dispatch_intent(intent: Intent) {
    let mut bus = INTENT_BUS.lock();
    bus.push(intent);
}

pub fn pop_intent() -> Option<Intent> {
    let mut bus = INTENT_BUS.lock();
    if !bus.is_empty() {
        Some(bus.remove(0))
    } else {
        None
    }
}
