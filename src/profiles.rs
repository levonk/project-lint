use crate::config::{MatchPosition, Profile, ProfileActivation};
use crate::hooks::ProjectLintEvent;
use crate::utils::Result;
use glob::glob;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use tracing::{debug, warn};

const HEADER_SIZE: usize = 1024;

pub fn is_profile_active(
    project_path: &Path,
    profile: &Profile,
    event: Option<&ProjectLintEvent>,
) -> Result<bool> {
    let activation = &profile.activation;

    // Check event-based activation
    if let Some(event) = event {
        for event_trigger in &activation.events {
            // Match against unified event type (string representation)
            let event_type_str = serde_json::to_string(&event.event_type)?
                .trim_matches('"')
                .to_string();

            if event_trigger == &event_type_str || event_trigger == "all" {
                debug!(
                    "Profile '{}' activated by event: {}",
                    profile.metadata.name, event_type_str
                );
                return Ok(true);
            }

            // Also check for IDE-specific events if specified
            if let Some(original_payload) = &event.context.original_payload {
                // Windsurf specific
                if let Some(action_name) = original_payload["agent_action_name"].as_str() {
                    if event_trigger == action_name {
                        debug!(
                            "Profile '{}' activated by Windsurf event: {}",
                            profile.metadata.name, action_name
                        );
                        return Ok(true);
                    }
                }
                // Claude specific
                if let Some(hook_event_name) = original_payload["hook_event_name"].as_str() {
                    if event_trigger == hook_event_name {
                        debug!(
                            "Profile '{}' activated by Claude event: {}",
                            profile.metadata.name, hook_event_name
                        );
                        return Ok(true);
                    }
                }
            }
        }
    }

    // Check indicators (files that must exist)
    for indicator in &activation.indicators {
        if project_path.join(indicator).exists() {
            debug!(
                "Profile '{}' activated by indicator: {}",
                profile.metadata.name, indicator
            );
            return Ok(true);
        }
    }

    // Check paths (directories that must exist)
    for path in &activation.paths {
        if project_path.join(path).exists() {
            debug!(
                "Profile '{}' activated by path: {}",
                profile.metadata.name, path
            );
            return Ok(true);
        }
    }

    // Check globs (file patterns)
    for pattern in &activation.globs {
        let full_pattern = project_path.join(pattern);
        let pattern_str = full_pattern.to_string_lossy();

        match glob(&pattern_str) {
            Ok(paths) => {
                // If we find at least one match, the profile is active
                if paths.count() > 0 {
                    debug!(
                        "Profile '{}' activated by glob: {}",
                        profile.metadata.name, pattern
                    );
                    return Ok(true);
                }
            }
            Err(e) => {
                warn!("Invalid glob pattern '{}': {}", pattern, e);
            }
        }
    }

    // Check content triggers
    for trigger in &activation.content {
        let patterns_to_check = if trigger.globs.is_empty() {
            vec!["**/*".to_string()]
        } else {
            trigger.globs.clone()
        };

        for glob_pattern in patterns_to_check {
            let full_pattern = project_path.join(&glob_pattern);
            let pattern_str = full_pattern.to_string_lossy();

            if let Ok(paths) = glob(&pattern_str) {
                for path_result in paths {
                    if let Ok(path) = path_result {
                        if path.is_file() {
                            if check_file_content(&path, &trigger.matches, &trigger.position) {
                                debug!(
                                    "Profile '{}' activated by content match in: {:?}",
                                    profile.metadata.name, path
                                );
                                return Ok(true);
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(false)
}

fn check_file_content(path: &Path, matches: &[String], position: &MatchPosition) -> bool {
    match File::open(path) {
        Ok(mut file) => {
            let mut buffer = Vec::new();

            if *position == MatchPosition::Header {
                let mut head_buf = [0u8; HEADER_SIZE];
                match file.read(&mut head_buf) {
                    Ok(n) => {
                        buffer.extend_from_slice(&head_buf[..n]);
                    }
                    Err(_) => return false,
                }
            } else {
                // For 'Any', we read the whole file.
                // TODO: Add size limit check to prevent OOM on huge files
                if file.read_to_end(&mut buffer).is_err() {
                    return false;
                }
            }

            // Convert buffer to string (lossy) for searching
            let content = String::from_utf8_lossy(&buffer);

            for pattern in matches {
                if content.contains(pattern) {
                    return true;
                }
            }
            false
        }
        Err(_) => false,
    }
}

pub fn get_active_profiles(
    project_path: &Path,
    available_profiles: &[Profile],
    event: Option<&ProjectLintEvent>,
) -> Result<Vec<Profile>> {
    let mut active_profiles = Vec::new();

    for profile in available_profiles {
        if is_profile_active(project_path, profile, event)? {
            active_profiles.push(profile.clone());
        }
    }

    Ok(active_profiles)
}
