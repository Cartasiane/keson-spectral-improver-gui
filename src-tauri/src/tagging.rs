use lofty::config::WriteOptions;
use lofty::prelude::*;
use lofty::probe::Probe;
use lofty::tag::Tag;
use std::path::Path;

/// Tag key used to mark files as replaced by Keson
const KESON_TAG_KEY: &str = "KESON_REPLACED";

/// Write the KESON_REPLACED tag to an audio file.
/// Returns Ok(true) if successful, Ok(false) if file format not supported.
pub fn write_replaced_tag(path: &Path) -> Result<bool, String> {
    let mut tagged_file = match Probe::open(path) {
        Ok(probe) => match probe.read() {
            Ok(file) => file,
            Err(e) => return Err(format!("Failed to read file: {}", e)),
        },
        Err(e) => return Err(format!("Failed to open file: {}", e)),
    };

    // Get or create the primary tag
    let tag = match tagged_file.primary_tag_mut() {
        Some(t) => t,
        None => {
            if let Some(first_tag) = tagged_file.first_tag_mut() {
                first_tag
            } else {
                // Create a new tag
                let tag_type = tagged_file.primary_tag_type();
                tagged_file.insert_tag(Tag::new(tag_type));
                match tagged_file.primary_tag_mut() {
                    Some(t) => t,
                    None => return Ok(false), // Format doesn't support tags
                }
            }
        }
    };

    // Get current timestamp
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    // Set the KESON_REPLACED tag as a comment
    // Using set_comment for broad compatibility across formats
    let existing_comment = tag.comment().unwrap_or_default().to_string();
    let new_comment = if existing_comment.contains(KESON_TAG_KEY) {
        // Update existing tag
        let re = regex::Regex::new(&format!(r"{}=[^\n]*", KESON_TAG_KEY)).unwrap();
        re.replace(&existing_comment, &format!("{}={}", KESON_TAG_KEY, timestamp))
            .to_string()
    } else if existing_comment.is_empty() {
        format!("{}={}", KESON_TAG_KEY, timestamp)
    } else {
        format!("{}\n{}={}", existing_comment, KESON_TAG_KEY, timestamp)
    };

    tag.set_comment(new_comment);

    // Save back to file
    tag.save_to_path(path, WriteOptions::default())
        .map_err(|e| format!("Failed to save tag: {}", e))?;

    println!("[tagging] Wrote KESON_REPLACED tag to: {:?}", path);
    Ok(true)
}

/// Check if an audio file has the KESON_REPLACED tag.
/// Returns Ok(true) if tagged, Ok(false) if not tagged or not supported.
pub fn has_replaced_tag(path: &Path) -> bool {
    let tagged_file = match Probe::open(path) {
        Ok(probe) => match probe.read() {
            Ok(file) => file,
            Err(_) => return false,
        },
        Err(_) => return false,
    };

    // Check primary tag first, then any tag
    if let Some(tag) = tagged_file.primary_tag() {
        if let Some(comment) = tag.comment() {
            if comment.contains(KESON_TAG_KEY) {
                return true;
            }
        }
    }

    // Check all tags
    for tag in tagged_file.tags() {
        if let Some(comment) = tag.comment() {
            if comment.contains(KESON_TAG_KEY) {
                return true;
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_has_replaced_tag_nonexistent() {
        let path = PathBuf::from("/nonexistent/file.mp3");
        assert!(!has_replaced_tag(&path));
    }
}
