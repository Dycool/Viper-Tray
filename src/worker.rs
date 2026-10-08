use crate::{
    config::{self, Config, Lighting, Preset},
    device::{Mouse, Snapshot},
    protocol::{DpiStages, Result},
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicIsize, Ordering},
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
pub const STATE_CHANGED: u32 = windows_sys::Win32::UI::WindowsAndMessaging::WM_APP + 3;

pub struct Worker {
    window: Arc<AtomicIsize>,
    pub state: Arc<Mutex<State>>,
    pub tx: Sender<Command>,
    pub interval: Arc<Mutex<u64>>,
    join: Option<thread::JoinHandle<()>>,
}
impl Worker {
    pub fn start(config: &Config) -> Self {
        let window = Arc::new(AtomicIsize::new(0));
        let notify_window = window.clone();
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
                        notify(&notify_window);
                    }
                    let result = Mouse::open().map(|m| {
                        let result = apply(&m, &c);
                        let previous = s.lock().unwrap().mouse.clone();
                        let mut snap = if matches!(c, Command::Refresh)
                            || previous.connection != m.connection
                            || !previous.accessible
                            || result.is_err()
                        {
                            m.snapshot()
                        } else {
                            previous
                        };
                        snap.note.clear();
                        match result {
                            Ok(()) => {
                                update_confirmed(&mut snap, &c);
                                // DPI and stage selection can affect each other.
                                let related = match c {
                                    Command::Dpi(_) => m.stages().map(|v| snap.stages = Some(v)),
                                    Command::Stages(_) | Command::Preset(_) => {
                                        m.dpi().map(|v| snap.dpi = Some(v))
                                    }
                                    _ => Ok(()),
                                };
                                if let Err(e) = related {
                                    config::log(&e);
                                    snap.error = Some(e);
                                }
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
                            config::log(&e);
                            st.mouse.accessible = false;
                            st.mouse.note.clear();
                            st.mouse.error = Some(e);
                        }
                    }
                    st.busy = false;
                    st.revision += 1;
                    notify(&notify_window);
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
                            notify(&notify_window);
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
                            notify(&notify_window);
                        }
                        Err(e) => {
                            let mut st = s.lock().unwrap();
                            if st.mouse.error.as_ref() != Some(&e) {
                                st.mouse.error = Some(e);
                                st.revision += 1;
                                notify(&notify_window);
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
                        notify(&notify_window);
                    }
                    Err(e) => {
                        failures = failures.saturating_add(1);
                        let mut st = s.lock().unwrap();
                        st.mouse.error = Some(e.clone());
                        st.revision += 1;
                        notify(&notify_window);
                        config::log(&e);
                    }
                }
                next_battery = Instant::now() + Duration::from_secs(backoff(every, failures));
            }
        });
        Self {
            window,
            state,
            tx,
            interval,
            join: Some(join),
        }
    }
    pub fn set_window(&self, hwnd: windows_sys::Win32::Foundation::HWND) {
        self.window.store(hwnd as isize, Ordering::Release);
        notify(&self.window);
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
fn notify(window: &AtomicIsize) {
    let hwnd = window.load(Ordering::Acquire) as windows_sys::Win32::Foundation::HWND;
    if !hwnd.is_null() {
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(hwnd, STATE_CHANGED, 0, 0);
        }
    }
}
// Setters already validate readback. Keep unrelated cached settings instead of
// issuing a complete nine-command snapshot after every individual change.
fn update_confirmed(s: &mut Snapshot, c: &Command) {
    if !matches!(c, Command::Refresh) {
        s.error = None;
    }
    match c {
        Command::Polling(v) => s.polling = Some(*v),
        Command::Idle(v) => s.idle = Some(*v),
        Command::Threshold(v) => {
            s.threshold = Some(((*v as u16 * 255 + 50) / 100).clamp(12, 63) as u8)
        }
        Command::Brightness(v) => s.brightness = Some(((*v as u16 * 255 + 50) / 100) as u8),
        Command::Dpi(v) => s.dpi = Some(*v),
        Command::Stages(v) => s.stages = Some(v.clone()),
        Command::Preset(p) => {
            s.polling = Some(p.polling);
            s.idle = Some(p.idle);
            s.threshold = Some(p.threshold);
            s.brightness = Some(p.brightness);
            s.dpi = Some(p.dpi);
            s.stages = Some(p.stages.clone());
        }
        _ => {}
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
    fn confirmed_changes_preserve_unrelated_cached_settings() {
        let mut s = Snapshot {
            polling: Some(1000),
            idle: Some(900),
            threshold: Some(13),
            brightness: Some(7),
            dpi: Some([3200, 2400]),
            battery: Some(91),
            error: Some("Previous command failed".into()),
            ..Default::default()
        };
        update_confirmed(&mut s, &Command::Polling(500));
        assert_eq!(s.polling, Some(500));
        assert_eq!(s.threshold, Some(13));
        assert_eq!(s.brightness, Some(7));
        assert_eq!(s.dpi, Some([3200, 2400]));
        assert_eq!(s.battery, Some(91));
        assert!(s.error.is_none());
        update_confirmed(&mut s, &Command::Threshold(5));
        assert_eq!(s.threshold, Some(13));
        update_confirmed(&mut s, &Command::Brightness(10));
        assert_eq!(s.brightness, Some(26));
        s.error = Some("Read failed".into());
        update_confirmed(&mut s, &Command::Refresh);
        assert_eq!(s.error.as_deref(), Some("Read failed"));
    }
    #[test]
    #[ignore = "Requires an explicitly authorized, awake wireless Viper Ultimate"]
    fn wireless_unchanged_polling_latency() {
        let mut worker = Worker::start(&Config {
            battery_interval: 0,
            ..Default::default()
        });
        let deadline = Instant::now() + Duration::from_secs(10);
        let original = loop {
            let st = worker.state.lock().unwrap();
            if st.mouse.accessible && !st.busy {
                break st.mouse.clone();
            }
            drop(st);
            assert!(
                Instant::now() < deadline,
                "Wireless settings could not be read"
            );
            thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(
            original.connection, "Wireless",
            "Test requires wireless connection"
        );
        let polling = original.polling.expect("Polling unavailable");
        for _ in 0..3 {
            let revision = worker.state.lock().unwrap().revision;
            let started = Instant::now();
            worker.tx.send(Command::Polling(polling)).unwrap();
            loop {
                let st = worker.state.lock().unwrap();
                if st.revision >= revision + 2 && !st.busy {
                    assert!(st.mouse.error.is_none(), "{:?}", st.mouse.error);
                    assert_eq!(st.mouse.polling, Some(polling));
                    assert_eq!(st.mouse.dpi, original.dpi);
                    assert_eq!(st.mouse.idle, original.idle);
                    assert_eq!(st.mouse.brightness, original.brightness);
                    println!(
                        "Wireless unchanged polling write + readback: {} ms",
                        started.elapsed().as_millis()
                    );
                    break;
                }
                drop(st);
                assert!(
                    started.elapsed() < Duration::from_secs(10),
                    "Setting timed out"
                );
                thread::sleep(Duration::from_millis(5));
            }
        }
        worker.stop();
    }
    #[test]
    fn timeout_backoff_is_bounded() {
        assert_eq!(backoff(60, 0), 60);
        assert_eq!(backoff(60, 1), 120);
        assert_eq!(backoff(60, 2), 240);
        assert_eq!(backoff(300, 8), 900);
    }
}
