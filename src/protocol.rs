// SPDX-License-Identifier: GPL-2.0-or-later
// Derived from OpenRazer driver/razercommon.c, razerchromacommon.c,
// razermouse_driver.c. See THIRD_PARTY_NOTICES.md for attribution.
use serde::{Deserialize, Serialize};
pub type Result<T> = std::result::Result<T, String>;

pub fn report(tid: u8, class: u8, command: u8, args: &[u8], size: usize) -> Result<[u8; 91]> {
    if size > 80 || args.len() > size {
        return Err("Invalid report payload".into());
    }
    let mut b = [0; 91];
    b[2] = tid;
    b[6] = size as u8;
    b[7] = class;
    b[8] = command;
    b[9..9 + args.len()].copy_from_slice(args);
    b[89] = b[3..89].iter().fold(0, |v, n| v ^ n);
    Ok(b)
}

pub fn response(req: &[u8; 91], data: &[u8]) -> Result<Option<Vec<u8>>> {
    if data.len() != 91 || data[0] != 0 {
        return Err("Invalid HID report length or ID".into());
    }
    if data[2..5] != req[2..5] || data[7..9] != req[7..9] || data[6] != req[6] {
        return Err("Mouse response does not match request".into());
    }
    if data[3..89].iter().fold(0, |v, n| v ^ n) != data[89] {
        return Err("Mouse response checksum failed".into());
    }
    match data[1] {
        1 => Ok(None),
        2 => Ok(Some(data[9..9 + data[6] as usize].to_vec())),
        3 => Err("Mouse rejected the setting".into()),
        4 => Err("Mouse did not respond. Try the USB cable or wake the mouse.".into()),
        5 => Err("Mouse firmware does not support this command".into()),
        n => Err(format!("Unexpected mouse status {n}")),
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DpiStages {
    pub active: u8,
    pub values: Vec<[u16; 2]>,
}
impl DpiStages {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.values.is_empty()
            || self.values.len() > 5
            || self.active == 0
            || self.active as usize > self.values.len()
        {
            return Err("Choose 1–5 DPI stages and an active stage within that range".into());
        }
        let mut b = vec![0; 38];
        b[0] = 1;
        b[1] = self.active;
        b[2] = self.values.len() as u8;
        for (i, xy) in self.values.iter().enumerate() {
            if xy.iter().any(|v| !(100..=20000).contains(v)) {
                return Err("DPI must be between 100 and 20,000".into());
            }
            let n = 3 + i * 7;
            b[n] = i as u8;
            b[n + 1..n + 3].copy_from_slice(&xy[0].to_be_bytes());
            b[n + 3..n + 5].copy_from_slice(&xy[1].to_be_bytes());
        }
        Ok(b)
    }
    pub fn decode(b: &[u8]) -> Result<Self> {
        if b.len() != 38 || !(1..=5).contains(&b[2]) {
            return Err("Invalid DPI stage response".into());
        }
        let values = (0..b[2] as usize)
            .map(|i| {
                let n = 3 + i * 7;
                [
                    u16::from_be_bytes([b[n + 1], b[n + 2]]),
                    u16::from_be_bytes([b[n + 3], b[n + 4]]),
                ]
            })
            .collect();
        let result = Self {
            active: b[1],
            values,
        };
        result.encode()?;
        Ok(result)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Effect {
    Off,
    Static,
    Spectrum,
    Breathing,
    BreathingDual,
    BreathingRandom,
    Reactive,
}
impl Effect {
    pub fn args(self, color: [u8; 3], second: [u8; 3], speed: u8) -> Vec<u8> {
        let (id, size) = match self {
            Self::Off => (0, 6),
            Self::Static => (1, 9),
            Self::Spectrum => (3, 6),
            Self::Reactive => (5, 9),
            Self::Breathing => (2, 9),
            Self::BreathingDual => (2, 12),
            Self::BreathingRandom => (2, 6),
        };
        let mut b = vec![0; size];
        b[0] = 1;
        b[1] = 4;
        b[2] = id;
        match self {
            Self::Static => {
                b[5] = 1;
                b[6..9].copy_from_slice(&color);
            }
            Self::Reactive => {
                b[4] = speed.clamp(1, 4);
                b[5] = 1;
                b[6..9].copy_from_slice(&color);
            }
            Self::Breathing | Self::BreathingDual => {
                let count = if self == Self::BreathingDual { 2 } else { 1 };
                b[3] = count;
                b[5] = count;
                b[6..9].copy_from_slice(&color);
                if count == 2 {
                    b[9..12].copy_from_slice(&second);
                }
            }
            _ => {}
        }
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn polling_packet_matches_openrazer() {
        let b = report(0xff, 0, 5, &[1], 1).unwrap();
        assert_eq!(&b[1..10], &[0, 255, 0, 0, 0, 1, 0, 5, 1]);
        assert_eq!(b[89], 5);
    }
    #[test]
    fn malformed_and_timeout_responses_are_not_success() {
        let q = report(255, 7, 0x83, &[], 2).unwrap();
        let mut r = q;
        r[1] = 2;
        assert_eq!(response(&q, &r).unwrap(), Some(vec![0, 0]));
        r[1] = 4;
        assert!(response(&q, &r).is_err());
        r[1] = 1;
        assert_eq!(response(&q, &r).unwrap(), None);
        r[9] = 3;
        assert!(response(&q, &r).is_err());
        assert!(response(&q, &r[..90]).is_err());
    }
    #[test]
    fn dpi_stages_preserve_separate_axes() {
        let s = DpiStages {
            active: 2,
            values: vec![[800, 900], [1600, 1700]],
        };
        assert_eq!(DpiStages::decode(&s.encode().unwrap()).unwrap(), s);
    }
    #[test]
    fn dpi_validation_rejects_out_of_range_and_invalid_active() {
        assert!(
            DpiStages {
                active: 0,
                values: vec![[800, 800]]
            }
            .encode()
            .is_err()
        );
        assert!(
            DpiStages {
                active: 1,
                values: vec![[20001, 800]]
            }
            .encode()
            .is_err()
        );
    }
    #[test]
    fn lighting_wire_formats() {
        assert_eq!(
            Effect::Static.args([1, 2, 3], [0; 3], 2),
            vec![1, 4, 1, 0, 0, 1, 1, 2, 3]
        );
        assert_eq!(
            Effect::BreathingDual.args([1, 2, 3], [4, 5, 6], 2),
            vec![1, 4, 2, 2, 0, 2, 1, 2, 3, 4, 5, 6]
        );
    }
}
