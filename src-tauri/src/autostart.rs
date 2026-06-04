use std::fs;
use std::path::PathBuf;

use crate::errors::{WorkerError, WorkerResult};

const PLIST_NAME: &str = "com.localworker.desktop.plist";

pub fn is_enabled() -> bool {
    plist_path().map(|path| path.exists()).unwrap_or(false)
}

pub fn set_enabled(enabled: bool) -> WorkerResult<bool> {
    let path = plist_path()?;
    if enabled {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let exe = std::env::current_exe()?;
        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>com.localworker.desktop</string>
  <key>ProgramArguments</key>
  <array>
    <string>{}</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <false/>
</dict>
</plist>
"#,
            escape_xml(&exe.to_string_lossy())
        );
        fs::write(&path, plist)?;
    } else if path.exists() {
        fs::remove_file(&path)?;
    }
    Ok(is_enabled())
}

fn plist_path() -> WorkerResult<PathBuf> {
    let home = std::env::var("HOME").map_err(|_| {
        WorkerError::Message("HOME is not set; cannot configure autostart".to_string())
    })?;
    Ok(PathBuf::from(home)
        .join("Library")
        .join("LaunchAgents")
        .join(PLIST_NAME))
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
