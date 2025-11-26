use crate::config::{MatchPosition, Profile, ProfileActivation};
use crate::utils::Result;
use glob::glob;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use tracing::{debug, warn};

const HEADER_SIZE: usize = 1024;

pub fn is_profile_active(project_path: &Path, profile: &Profile) -> Result<bool> {
    let activation = &profile.activation;

    // Check indicators (files that must exist)
    for indicator in &activation.indicators {
        if project_path.join(indicator).exists() {
            debug!("Profile '{}' activated by indicator: {}", profile.metadata.name, indicator);
            return Ok(true);
        }
    }

    // Check paths (directories that must exist)
    for path in &activation.paths {
        if project_path.join(path).exists() {
            debug!("Profile '{}' activated by path: {}", profile.metadata.name, path);
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
                    debug!("Profile '{}' activated by glob: {}", profile.metadata.name, pattern);
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
                                debug!("Profile '{}' activated by content match in: {:?}", profile.metadata.name, path);
                                return Ok(true);
                            }
                        }
                    }
                }
            }
        }
    }

    // Note: Git branch checking requires GitInfo which is not passed here.
    // This function focuses on file-system based activation.
    
    // Backward compatibility for extensions (if still used)
    if !activation.extensions.is_empty() {
        // This would require walking the directory to check for extensions, 
        // which might be expensive. For now, we rely on globs.
        // Use globs instead of extensions in profiles.
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

pub fn get_active_profiles(project_path: &Path, available_profiles: &[Profile]) -> Result<Vec<Profile>> {
    let mut active_profiles = Vec::new();

    for profile in available_profiles {
        if is_profile_active(project_path, profile)? {
            active_profiles.push(profile.clone());
        }
    }

    Ok(active_profiles)
}
