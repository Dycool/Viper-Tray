use crate::{
    config::{self, Config, Lighting, Preset},
    device::{Mouse, Snapshot},
    protocol::{DpiStages, Result},
};
use std::{
    sync::{
        Arc, Mutex,
        mpsc::{self, Sender},
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
pub enum Command {
    Refresh,
    Polling(u16),
    Idle(u16),
    Threshold(u8),
    Brightness(u8),
    Dpi([u16; 2]),
    Stages(DpiStages),
    Lighting(Lighting),
    Preset(Preset),
    Quit,
}
#[derive(Default)]
pub struct State {
    pub mouse: Snapshot,
    pub busy: bool,
    pub revision: u64,
    pub updated: Option<Instant>,
}
pub struct Worker {
    pub state: Arc<Mutex<State>>,
    pub tx: Sender<Command>,
    pub interval: Arc<Mutex<u64>>,
    join: Option<thread::JoinHandle<()>>,
}
impl Worker {
    pub fn start(config: &Config) -> Self {
        let state = Arc::new(Mutex::new(State::default()));
        let interval = Arc::new(Mutex::new(config.battery_interval));
        let (tx, rx) = mpsc::channel();
        let s = state.clone();
        let poll = interval.clone();
        let join = thread::spawn(move || {
            let mut connection = String::new();
            let mut next_battery = Instant::now();
            let mut detect = Instant::now();
            let mut failures = 0u32;
            loop {
                let command = match rx.recv_timeout(Duration::from_millis(500)) {
                    Ok(Command::Quit) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Ok(c) => Some(c),
                    Err(_) => None,
                };
                if let Some(c) = command {
                    {
                        let mut st = s.lock().unwrap();
                        st.busy = true;
                        st.revision += 1;
                    }
                    let result = Mouse::open().map(|m| {
                        let result = apply(&m, &c);
                        let mut snap = m.snapshot();
                        match result {
                            Ok(()) => {
                                snap.note = if matches!(c, Command::Refresh) {
                                    "Settings refreshed".into()
                                } else {
                                    "Setting confirmed by mouse".into()
                                }
                            }
                            Err(e) => {
                                config::log(&e);
                                snap.error = Some(e);
                            }
                        }
                        snap
                    });
                    let mut st = s.lock().unwrap();
                    match result {
                        Ok(snap) => {
                            connection = snap.connection.clone();
                            st.mouse = snap;
                            st.updated = Some(Instant::now());
                        }
                        Err(e) => {
                            st.mouse.accessible = false;
                            st.mouse.error = Some(e);
                        }
                    }
                    st.busy = false;
                    st.revision += 1;
                    next_battery = Instant::now() + Duration::from_secs(*poll.lock().unwrap());
                    continue;
                }
                if Instant::now() >= detect {
                    detect = Instant::now() + Duration::from_secs(10);
                    match Mouse::open() {
                        Ok(m) if m.connection != connection => {
                            connection = m.connection.clone();
                            let snap = m.snapshot();
                            let mut st = s.lock().unwrap();
                            st.mouse = snap;
                            st.updated = Some(Instant::now());
                            st.revision += 1;
                            next_battery = Instant::now() + Duration::from_secs(60);
                            failures = 0;
                        }
                        Err(e) if !connection.is_empty() => {
                            connection.clear();
                            let mut st = s.lock().unwrap();
                            st.mouse = Snapshot {
                                error: Some(e),
                                ..Default::default()
                            };
                            st.updated = None;
                            st.revision += 1;
                        }
                        Err(e) => {
                            let mut st = s.lock().unwrap();
                            if st.mouse.error.as_ref() != Some(&e) {
                                st.mouse.error = Some(e);
                                st.revision += 1;
                            }
                        }
                        _ => {}
                    }
                }
                let every = *poll.lock().unwrap();
                if every == 0 || connection.is_empty() || Instant::now() < next_battery {
                    continue;
                }
                let idle = input_idle_seconds();
                let sleep = s.lock().unwrap().mouse.idle.unwrap_or(900) as u64;
                // Battery queries are deferred during input and after the mouse's idle sleep time.
                if idle < 2 || idle >= sleep.max(60) {
                    continue;
                }
                match Mouse::open().and_then(|m| m.battery()) {
                    Ok((battery, charging)) => {
                        failures = 0;
                        let mut st = s.lock().unwrap();
                        st.mouse.battery = Some(battery);
                        st.mouse.charging = Some(charging);
                        st.revision += 1;
                    }
                    Err(e) => {
                        failures = failures.saturating_add(1);
                        let mut st = s.lock().unwrap();
                        st.mouse.error = Some(e.clone());
                        st.revision += 1;
                        config::log(&e);
                    }
                }
                next_battery = Instant::now() + Duration::from_secs(backoff(every, failures));
            }
        });
        Self {
            state,
            tx,
            interval,
            join: Some(join),
        }
    }
    pub fn stop(&mut self) {
        let _ = self.tx.send(Command::Quit);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.stop();
    }
}
pub fn apply(m: &Mouse, c: &Command) -> Result<()> {
    match c {
        Command::Refresh | Command::Quit => Ok(()),
        Command::Polling(v) => m.set_polling(*v),
        Command::Idle(v) => m.set_idle(*v),
        Command::Threshold(v) => m.set_threshold(*v),
        Command::Brightness(v) => m.set_brightness(*v),
        Command::Dpi(v) => m.set_dpi(*v),
        Command::Stages(v) => m.set_stages(v),
        Command::Lighting(v) => m.effect(v.effect, v.color, v.second, v.speed),
        Command::Preset(p) => {
            // Validate the complete preset before any write, then stop at the first failure.
            p.stages.encode()?;
            if ![125, 500, 1000].contains(&p.polling)
                || !(60..=900).contains(&p.idle)
                || !(12..=63).contains(&p.threshold)
                || p.dpi.iter().any(|v| !(100..=20000).contains(v))
            {
                return Err("Preset contains an invalid setting".into());
            }
            m.set_polling(p.polling)?;
            m.set_idle(p.idle)?;
            m.set_threshold_raw(p.threshold)?;
            m.set_stages(&p.stages)?;
            m.set_dpi(p.dpi)?;
            m.set_brightness_raw(p.brightness)?;
            if let Some(l) = &p.lighting {
                m.effect(l.effect, l.color, l.second, l.speed)?;
            }
            Ok(())
        }
    }
}
fn backoff(interval: u64, failures: u32) -> u64 {
    if failures == 0 {
        interval.max(60)
    } else {
        interval
            .max(60)
            .saturating_mul(1u64 << failures.min(4))
            .min(900)
    }
}
fn input_idle_seconds() -> u64 {
    use windows_sys::Win32::{
        System::SystemInformation::GetTickCount,
        UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO},
    };
    let mut info = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    unsafe {
        if GetLastInputInfo(&mut info) == 0 {
            return 0;
        }
        GetTickCount().wrapping_sub(info.dwTime) as u64 / 1000
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timeout_backoff_is_bounded() {
        assert_eq!(backoff(60, 0), 60);
        assert_eq!(backoff(60, 1), 120);
        assert_eq!(backoff(60, 2), 240);
        assert_eq!(backoff(300, 8), 900);
    }
}
