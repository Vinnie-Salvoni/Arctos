// =============================================================================
// Arctos OS — Per-User Profile & Persistent Hotspot Guidance System
// =============================================================================

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use spin::Mutex;
use crate::fs;
use crate::intent;

/// Stores personalized user workspace preferences, RAM allocation limits, and corner hotspots.
#[derive(Debug, Clone)]
pub struct UserProfile {
    pub username: String,
    pub role: String,
    pub ram_limit_mb: usize,
    pub theme_primary: u32,
    pub theme_secondary: Option<u32>,
    pub active_surface: String,
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

impl Default for UserProfile {
    fn default() -> Self {
        UserProfile {
            username: "DefaultUser".to_string(),
            role: "Software Developer".to_string(),
            ram_limit_mb: 2048, // 2 GB RAM limit allowed
            theme_primary: 0x000F2A38,
            theme_secondary: None,
            active_surface: "developer".to_string(),
            show_telemetry: true,
            is_first_boot: true,
            corner_top_left_label: "[ Dev ]".to_string(),
            corner_top_left_action: "dev".to_string(),
            corner_top_right_label: "[ AI ]".to_string(),
            corner_top_right_action: "ai".to_string(),
            corner_bottom_left_label: "[ Pin ]".to_string(),
            corner_bottom_left_action: "pin Quick dev".to_string(),
            corner_bottom_right_label: "[ Exit ]".to_string(),
            corner_bottom_right_action: "exit".to_string(),
        }
    }
}

pub static CURRENT_USER_PROFILE: Mutex<Option<UserProfile>> = Mutex::new(None);

/// Loads user profile preferences from AIVFS storage (`user_profile.cfg`).
/// If no profile exists, creates a default profile and saves it.
pub fn get_or_load_profile() -> UserProfile {
    let mut profile_guard = CURRENT_USER_PROFILE.lock();
    if let Some(ref profile) = *profile_guard {
        return profile.clone();
    }

    // Auto-mount storage if needed
    if !fs::mount() {
        fs::format();
    }

    let loaded_profile = if let Some(data) = fs::read_file("user_profile.cfg") {
        if let Ok(text) = core::str::from_utf8(&data) {
            parse_profile_config(text)
        } else {
            UserProfile::default()
        }
    } else {
        let default_prof = UserProfile::default();
        save_profile_file(&default_prof);
        default_prof
    };

    *profile_guard = Some(loaded_profile.clone());
    loaded_profile
}

/// Saves user profile configuration to AIVFS storage (`user_profile.cfg`).
pub fn save_profile(profile: &UserProfile) -> bool {
    let mut profile_guard = CURRENT_USER_PROFILE.lock();
    *profile_guard = Some(profile.clone());
    save_profile_file(profile)
}

fn save_profile_file(profile: &UserProfile) -> bool {
    if !fs::mount() {
        fs::format();
    }

    let sec_str = match profile.theme_secondary {
        Some(col) => alloc::format!("0x{:08X}", col),
        None => String::from("none"),
    };

    let cfg_text = alloc::format!(
        "username={}\nrole={}\nram_limit_mb={}\ntheme_primary=0x{:08X}\ntheme_secondary={}\nactive_surface={}\nshow_telemetry={}\nis_first_boot={}\ntl_label={}\ntl_action={}\ntr_label={}\ntr_action={}\nbl_label={}\nbl_action={}\nbr_label={}\nbr_action={}\n",
        profile.username,
        profile.role,
        profile.ram_limit_mb,
        profile.theme_primary,
        sec_str,
        profile.active_surface,
        profile.show_telemetry,
        profile.is_first_boot,
        profile.corner_top_left_label,
        profile.corner_top_left_action,
        profile.corner_top_right_label,
        profile.corner_top_right_action,
        profile.corner_bottom_left_label,
        profile.corner_bottom_left_action,
        profile.corner_bottom_right_label,
        profile.corner_bottom_right_action
    );

    fs::write_file("user_profile.cfg", cfg_text.as_bytes())
}

fn parse_profile_config(text: &str) -> UserProfile {
    let mut profile = UserProfile::default();

    for line in text.lines() {
        let parts: Vec<&str> = line.splitn(2, '=').collect();
        if parts.len() == 2 {
            let key = parts[0].trim();
            let val = parts[1].trim();
            match key {
                "username" => profile.username = val.to_string(),
                "role" => profile.role = val.to_string(),
                "ram_limit_mb" => profile.ram_limit_mb = val.parse::<usize>().unwrap_or(2048),
                "theme_primary" => {
                    if val.starts_with("0x") || val.starts_with("0X") {
                        if let Ok(c) = u32::from_str_radix(&val[2..], 16) {
                            profile.theme_primary = c;
                        }
                    } else if let Ok(c) = val.parse::<u32>() {
                        profile.theme_primary = c;
                    }
                }
                "theme_secondary" => {
                    if val == "none" || val.is_empty() {
                        profile.theme_secondary = None;
                    } else if val.starts_with("0x") || val.starts_with("0X") {
                        if let Ok(c) = u32::from_str_radix(&val[2..], 16) {
                            profile.theme_secondary = Some(c);
                        }
                    } else if let Ok(c) = val.parse::<u32>() {
                        profile.theme_secondary = Some(c);
                    }
                }
                "active_surface" => profile.active_surface = val.to_string(),
                "show_telemetry" => profile.show_telemetry = val.parse::<bool>().unwrap_or(true),
                "is_first_boot" => profile.is_first_boot = val.parse::<bool>().unwrap_or(true),
                "tl_label" => profile.corner_top_left_label = val.to_string(),
                "tl_action" => profile.corner_top_left_action = val.to_string(),
                "tr_label" => profile.corner_top_right_label = val.to_string(),
                "tr_action" => profile.corner_top_right_action = val.to_string(),
                "bl_label" => profile.corner_bottom_left_label = val.to_string(),
                "bl_action" => profile.corner_bottom_left_action = val.to_string(),
                "br_label" => profile.corner_bottom_right_label = val.to_string(),
                "br_action" => profile.corner_bottom_right_action = val.to_string(),
                _ => {}
            }
        }
    }

    profile
}

/// Applies user profile hotspots to the 1080p canvas as small corner nooks.
/// Applies user profile hotspots to the 1080p canvas as sleek corner nooks.
pub fn apply_profile_hotspots(profile: &UserProfile) {
    let mut spots = intent::HOTSPOTS.lock();
    spots.clear();

    // Top-Left Corner Nook (x=10, y=8, w=140, h=30)
    spots.push(intent::Hotspot {
        x: 10,
        y: 8,
        width: 140,
        height: 30,
        label: profile.corner_top_left_label.clone(),
        action: profile.corner_top_left_action.clone(),
    });

    // Top-Right Corner Nook (x=1710, y=8, w=140, h=30)
    spots.push(intent::Hotspot {
        x: 1710,
        y: 8,
        width: 140,
        height: 30,
        label: profile.corner_top_right_label.clone(),
        action: profile.corner_top_right_action.clone(),
    });

    // Bottom-Left Corner Nook (x=10, y=1042, w=140, h=30)
    spots.push(intent::Hotspot {
        x: 10,
        y: 1042,
        width: 140,
        height: 30,
        label: profile.corner_bottom_left_label.clone(),
        action: profile.corner_bottom_left_action.clone(),
    });

    // Bottom-Right Corner Nook (x=1770, y=1042, w=100, h=30)
    spots.push(intent::Hotspot {
        x: 1770,
        y: 1042,
        width: 100,
        height: 30,
        label: profile.corner_bottom_right_label.clone(),
        action: profile.corner_bottom_right_action.clone(),
    });
}


