use crate::protocol::{self, DpiStages, Effect, Result};
use hidapi::{HidApi, HidDevice};
use serde::{Deserialize, Serialize};
use std::{thread, time::Duration};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Snapshot {
    pub connection: String,
    pub accessible: bool,
    pub battery: Option<u8>,
    pub charging: Option<bool>,
    pub firmware: Option<String>,
    pub polling: Option<u16>,
    pub idle: Option<u16>,
    pub threshold: Option<u8>,
    pub brightness: Option<u8>,
    pub dpi: Option<[u16; 2]>,
    pub stages: Option<DpiStages>,
    pub error: Option<String>,
    pub note: String,
}

pub struct Mouse {
    dev: HidDevice,
    pub connection: String,
}
impl Mouse {
    pub fn open() -> Result<Self> {
        let api = HidApi::new().map_err(|e| e.to_string())?;
        for pid in [0x007a, 0x007b] {
            let devices: Vec<_> = api
                .device_list()
                .filter(|d| {
                    d.vendor_id() == 0x1532
                        && d.product_id() == pid
                        && d.interface_number() == 0
                        && d.usage() == 2
                })
                .collect();
            if devices.len() > 1 {
                return Err(
                    "Multiple Viper Ultimate devices found. Connect one mouse at a time.".into(),
                );
            }
            if let Some(d) = devices.first() {
                return Ok(Self {
                    dev: d.open_device(&api).map_err(|e| e.to_string())?,
                    connection: if pid == 0x007a {
                        "USB cable"
                    } else {
                        "Wireless"
                    }
                    .into(),
                });
            }
        }
        Err("Viper Ultimate not connected".into())
    }
    fn cmd(&self, tid: u8, class: u8, command: u8, args: &[u8], size: usize) -> Result<Vec<u8>> {
        let q = protocol::report(tid, class, command, args, size)?;
        self.dev
            .send_feature_report(&q)
            .map_err(|e| format!("USB write: {e}"))?;
        for attempt in 0..8 {
            // Try the acknowledgement promptly; back off only if the device is busy.
            thread::sleep(Duration::from_millis(if attempt == 0 { 10 } else { 80 }));
            let mut b = [0; 91];
            let n = self
                .dev
                .get_feature_report(&mut b)
                .map_err(|e| format!("USB read: {e}"))?;
            if let Some(r) = protocol::response(&q, &b[..n])? {
                return Ok(r);
            }
        }
        Err("Mouse stayed busy; no setting was confirmed".into())
    }
    pub fn polling(&self) -> Result<u16> {
        match self.cmd(255, 0, 0x85, &[], 1)?[0] {
            1 => Ok(1000),
            2 => Ok(500),
            8 => Ok(125),
            _ => Err("Unknown polling rate".into()),
        }
    }
    pub fn set_polling(&self, hz: u16) -> Result<()> {
        let v = match hz {
            1000 => 1,
            500 => 2,
            125 => 8,
            _ => return Err("Polling rate must be 125, 500, or 1,000 Hz".into()),
        };
        self.cmd(255, 0, 5, &[v], 1)?;
        self.verify(self.polling()?, hz, "polling rate")
    }
    pub fn idle(&self) -> Result<u16> {
        let b = self.cmd(255, 7, 0x83, &[], 2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }
    pub fn set_idle(&self, seconds: u16) -> Result<()> {
        if !(60..=900).contains(&seconds) {
            return Err("Idle sleep must be 1–15 minutes".into());
        }
        self.cmd(255, 7, 3, &seconds.to_be_bytes(), 2)?;
        self.verify(self.idle()?, seconds, "idle timer")
    }
    pub fn threshold(&self) -> Result<u8> {
        Ok(self.cmd(255, 7, 0x81, &[], 1)?[0])
    }
    pub fn set_threshold(&self, percent: u8) -> Result<()> {
        if !(5..=25).contains(&percent) {
            return Err("Threshold must be 5–25%".into());
        }
        let value = ((percent as u16 * 255 + 50) / 100).clamp(12, 63) as u8;
        self.set_threshold_raw(value)
    }
    pub fn set_threshold_raw(&self, value: u8) -> Result<()> {
        if !(12..=63).contains(&value) {
            return Err("Invalid battery threshold".into());
        }
        self.cmd(255, 7, 1, &[value], 1)?;
        self.verify(self.threshold()?, value, "battery threshold")
    }
    pub fn dpi(&self) -> Result<[u16; 2]> {
        let b = self.cmd(255, 4, 0x85, &[0], 7)?;
        Ok([
            u16::from_be_bytes([b[1], b[2]]),
            u16::from_be_bytes([b[3], b[4]]),
        ])
    }
    pub fn set_dpi(&self, xy: [u16; 2]) -> Result<()> {
        if xy.iter().any(|v| !(100..=20000).contains(v)) {
            return Err("DPI must be 100–20,000".into());
        }
        let mut b = vec![1];
        for v in xy {
            b.extend(v.to_be_bytes());
        }
        b.extend([0, 0]);
        self.cmd(255, 4, 5, &b, 7)?;
        self.verify(self.dpi()?, xy, "DPI")
    }
    pub fn stages(&self) -> Result<DpiStages> {
        DpiStages::decode(&self.cmd(255, 4, 0x86, &[1], 38)?)
    }
    pub fn set_stages(&self, stages: &DpiStages) -> Result<()> {
        self.cmd(255, 4, 6, &stages.encode()?, 38)?;
        self.verify(self.stages()?, stages.clone(), "DPI stages")
    }
    pub fn brightness(&self) -> Result<u8> {
        Ok(self.cmd(0x3f, 15, 0x84, &[1, 4], 3)?[2])
    }
    pub fn set_brightness(&self, percent: u8) -> Result<()> {
        if percent > 100 {
            return Err("Brightness must be 0–100%".into());
        }
        let v = ((percent as u16 * 255 + 50) / 100) as u8;
        self.set_brightness_raw(v)
    }
    pub fn set_brightness_raw(&self, v: u8) -> Result<()> {
        self.cmd(0x3f, 15, 4, &[1, 4, v], 3)?;
        self.verify(self.brightness()?, v, "brightness")
    }
    pub fn effect(&self, e: Effect, c: [u8; 3], second: [u8; 3], speed: u8) -> Result<()> {
        let args = e.args(c, second, speed);
        self.cmd(0x3f, 15, 2, &args, args.len())?;
        Ok(())
    }
    fn verify<T: PartialEq>(&self, actual: T, expected: T, label: &str) -> Result<()> {
        if actual == expected {
            Ok(())
        } else {
            Err(format!(
                "{label} write was acknowledged but readback did not match"
            ))
        }
    }
    pub fn battery(&self) -> Result<(u8, bool)> {
        let b = self.cmd(255, 7, 0x80, &[], 2)?;
        let charging = self.cmd(255, 7, 0x84, &[], 2)?;
        Ok((((b[1] as u16 * 100 + 127) / 255) as u8, charging[1] != 0))
    }
    pub fn snapshot(&self) -> Snapshot {
        let mut s = Snapshot {
            connection: self.connection.clone(),
            ..Default::default()
        };
        match self.polling() {
            Ok(v) => {
                s.polling = Some(v);
                s.accessible = true;
            }
            Err(e) => {
                s.error = Some(e);
                return s;
            }
        }
        let mut errors = vec![];
        macro_rules! field {
            ($f:ident,$v:expr) => {
                match $v {
                    Ok(v) => s.$f = Some(v),
                    Err(e) => errors.push(format!("{}: {e}", stringify!($f))),
                }
            };
        }
        field!(idle, self.idle());
        field!(threshold, self.threshold());
        field!(dpi, self.dpi());
        field!(stages, self.stages());
        field!(brightness, self.brightness());
        field!(
            firmware,
            self.cmd(255, 0, 0x81, &[], 2)
                .map(|b| format!("{}.{}", b[0], b[1]))
        );
        match self.battery() {
            Ok((b, c)) => {
                s.battery = Some(b);
                s.charging = Some(c);
            }
            Err(e) => errors.push(e),
        }
        if !errors.is_empty() {
            s.error = Some(errors.join("; "));
        }
        s
    }
}
