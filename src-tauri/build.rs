use std::env;
use std::fs;
use std::path::Path;

fn main() {
    // Tell Cargo to rerun if the updater feature changes
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_WITH_UPDATER");
    
    // Check if with-updater feature is enabled via environment variable
    let with_updater = env::var("CARGO_FEATURE_WITH_UPDATER").is_ok();
    
    let updater_capability = Path::new("capabilities/updater.json");
    let updater_capability_disabled = Path::new("capabilities/updater.json.disabled");

    if with_updater {
        // Enable updater capability for production builds
        if updater_capability_disabled.exists() && !updater_capability.exists() {
            fs::rename(updater_capability_disabled, updater_capability)
                .expect("Failed to enable updater capability");
            println!("cargo:warning=Enabled updater capability for production build");
        }
    } else {
        // Disable updater capability for dev builds
        if updater_capability.exists() {
            fs::rename(updater_capability, updater_capability_disabled)
                .expect("Failed to disable updater capability");
            println!("cargo:warning=Disabled updater capability for dev build");
        }
    }

    tauri_build::build()
}
