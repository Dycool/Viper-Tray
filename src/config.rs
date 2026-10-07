use crate::{
    device::Snapshot,
    protocol::{DpiStages, Effect, Result},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Preset {
    pub polling: u16,
    pub idle: u16,
    pub threshold: u8,
    pub brightness: u8,
    pub dpi: [u16; 2],
    pub stages: DpiStages,
    pub lighting: Option<Lighting>,
}
impl Preset {
    pub fn capture(s: &Snapshot, lighting: Option<Lighting>) -> Result<Self> {
        Ok(Self {
            polling: s.polling.ok_or("Polling unavailable")?,
            idle: s.idle.ok_or("Idle unavailable")?,
            threshold: s.threshold.ok_or("Threshold unavailable")?,
            brightness: s.brightness.ok_or("Brightness unavailable")?,
            dpi: s.dpi.ok_or("DPI unavailable")?,
            stages: s.stages.clone().ok_or("DPI stages unavailable")?,
            lighting,
        })
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Lighting {
    pub effect: Effect,
    pub color: [u8; 3],
    pub second: [u8; 3],
    pub speed: u8,
}
impl Default for Lighting {
    fn default() -> Self {
        Self {
            effect: Effect::Static,
            color: [0, 255, 0],
            second: [0, 128, 255],
            speed: 2,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub battery_interval: u64,
    pub presets: [Option<Preset>; 5],
    pub lighting: Option<Lighting>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            battery_interval: 60,
            presets: std::array::from_fn(|_| None),
            lighting: None,
        }
    }
}
pub fn directory() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("ViperTray")
}
impl Config {
    pub fn load() -> Result<Self> {
        let p = directory().join("settings.json");
        if !p.exists() {
            return Ok(Self::default());
        }
        let mut c: Self = serde_json::from_slice(&std::fs::read(p).map_err(|e| e.to_string())?)
            .map_err(|e| format!("Cannot load settings: {e}"))?;
        if ![0, 60, 120, 300].contains(&c.battery_interval) {
            c.battery_interval = 60;
        }
        Ok(c)
    }
    pub fn save(&self) -> Result<()> {
        let dir = directory();
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        let tmp = dir.join("settings.tmp");
        std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
        // MoveFileEx atomically replaces the old file on Windows.
        use std::os::windows::ffi::OsStrExt;
        let a: Vec<u16> = tmp.as_os_str().encode_wide().chain(Some(0)).collect();
        let b: Vec<u16> = dir
            .join("settings.json")
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        if unsafe {
            windows_sys::Win32::Storage::FileSystem::MoveFileExW(
                a.as_ptr(),
                b.as_ptr(),
                windows_sys::Win32::Storage::FileSystem::MOVEFILE_REPLACE_EXISTING
                    | windows_sys::Win32::Storage::FileSystem::MOVEFILE_WRITE_THROUGH,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(())
    }
}
pub fn log(message: &str) {
    use std::io::Write;
    let dir = directory();
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("diagnostics.log");
    if std::fs::metadata(&path)
        .map(|m| m.len() > 512_000)
        .unwrap_or(false)
    {
        let _ = std::fs::rename(&path, dir.join("diagnostics.previous.log"));
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let _ = writeln!(f, "{t}: {message}");
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn incomplete_snapshot_cannot_be_saved_as_preset() {
        assert!(Preset::capture(&Snapshot::default(), None).is_err());
    }
}
