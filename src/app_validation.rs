use super::*;
use crate::device::{Mouse, Snapshot};
use std::{
    thread,
    time::{Duration, Instant},
};

fn wait(app: &mut App, revision: u64) -> Snapshot {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        app.update();
        let st = app.worker.state.lock().unwrap();
        if st.revision >= revision + 2 && !st.busy && !app.pending {
            assert!(st.mouse.error.is_none(), "{:?}", st.mouse.error);
            return st.mouse.clone();
        }
        drop(st);
        assert!(Instant::now() < deadline, "Worker did not finish");
        thread::sleep(Duration::from_millis(5));
    }
}
fn execute(app: &mut App, action: Action) -> Snapshot {
    let revision = app.worker.state.lock().unwrap().revision;
    app.act(action);
    wait(app, revision)
}
fn inspect_menu(menu: HMENU, depth: usize, count: &mut usize) {
    assert!(depth < 10);
    unsafe {
        for i in 0..GetMenuItemCount(menu) as u32 {
            let mut text = [0u16; 512];
            let mut info = MENUITEMINFOW {
                cbSize: std::mem::size_of::<MENUITEMINFOW>() as u32,
                fMask: MIIM_STRING | MIIM_SUBMENU | MIIM_FTYPE | MIIM_ID,
                dwTypeData: text.as_mut_ptr(),
                cch: text.len() as u32,
                ..std::mem::zeroed()
            };
            assert_ne!(GetMenuItemInfoW(menu, i, 1, &mut info), 0);
            if info.fType & MFT_SEPARATOR == 0 {
                let label = String::from_utf16_lossy(&text[..info.cch as usize]);
                assert!(!label.is_empty(), "Empty native menu label");
                assert!(!label.contains("Settings read"));
                assert!(!label.to_lowercase().contains("diagnostic"));
                assert_eq!(info.fType & MFT_RIGHTORDER, 0);
                *count += 1;
            }
            if !info.hSubMenu.is_null() {
                inspect_menu(info.hSubMenu, depth + 1, count);
            }
        }
    }
}
struct Restore {
    preset: Preset,
    directory: std::path::PathBuf,
}
impl Drop for Restore {
    fn drop(&mut self) {
        match Mouse::open()
            .and_then(|m| crate::worker::apply(&m, &Command::Preset(self.preset.clone())))
        {
            Ok(()) => println!("Original mouse settings restored"),
            Err(e) => eprintln!("RESTORATION FAILED: {e}"),
        }
        *config::TEST_DIRECTORY.lock().unwrap() = None;
        // This exact unique directory was created by this test, never a user-selected path.
        if self
            .directory
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with("viper-validation-"))
        {
            let _ = std::fs::remove_dir_all(&self.directory);
        }
    }
}
#[test]
#[ignore = "Changes real wireless mouse settings; requires explicit hardware-test authorization"]
fn wireless_every_menu_control() {
    let mouse = Mouse::open().unwrap();
    assert_eq!(mouse.connection, "Wireless");
    let original = mouse.snapshot();
    assert!(original.error.is_none(), "{:?}", original.error);
    let original_config = Config::load().unwrap();
    assert!(
        original.brightness == Some(0) || original_config.lighting.is_some(),
        "Original lighting is unknown: test requires brightness zero or a known saved effect"
    );
    let preset = Preset::capture(&original, original_config.lighting).unwrap();
    let directory = std::env::temp_dir().join(format!(
        "viper-validation-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    let restore = Restore {
        preset: preset.clone(),
        directory: directory.clone(),
    };
    *config::TEST_DIRECTORY.lock().unwrap() = Some(directory);
    Config {
        battery_interval: 0,
        ..Default::default()
    }
    .save()
    .unwrap();
    let mut app = App::new();
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let st = app.worker.state.lock().unwrap();
        if st.mouse.accessible && st.mouse.polling.is_some() && !st.busy {
            break;
        }
        drop(st);
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(10));
    }
    app.worker.state.lock().unwrap().mouse = original.clone();
    let menu = app.build_menu();
    let mut labels = 0;
    inspect_menu(menu, 0, &mut labels);
    unsafe {
        DestroyMenu(menu);
    }
    println!(
        "Native menu: {labels} nonempty labels, nested structure, no diagnostics or forced-left flags"
    );
    let commands: Vec<Command> = app
        .actions
        .iter()
        .filter_map(|a| match a {
            Action::Device(c) => Some(c.clone()),
            _ => None,
        })
        .collect();
    println!("Testing {} hardware menu commands", commands.len());
    for (i, c) in commands.into_iter().enumerate() {
        let result = execute(&mut app, Action::Device(c.clone()));
        match &c {
            Command::Polling(v) => assert_eq!(result.polling, Some(*v)),
            Command::Idle(v) => assert_eq!(result.idle, Some(*v)),
            Command::Dpi(v) => assert_eq!(result.dpi, Some(*v)),
            Command::Stages(v) => assert_eq!(result.stages.as_ref(), Some(v)),
            Command::Lighting(v) => {
                assert_eq!(app.config.lighting.as_ref().unwrap().color, v.color)
            }
            _ => {}
        }
        if i % 25 == 0 {
            println!("Command {} passed: {c:?}", i + 1);
        }
    }
    execute(
        &mut app,
        Action::Device(Command::Lighting(Lighting {
            effect: crate::protocol::Effect::BreathingDual,
            color: [0, 255, 0],
            second: [255, 0, 0],
            speed: 2,
        })),
    );
    let menu = app.build_menu();
    unsafe {
        DestroyMenu(menu);
    }
    let dual_commands: Vec<Command> = app
        .actions
        .iter()
        .filter_map(|a| match a {
            Action::Device(Command::Lighting(l))
                if l.effect == crate::protocol::Effect::BreathingDual =>
            {
                Some(Command::Lighting(l.clone()))
            }
            _ => None,
        })
        .collect();
    let dual_count = dual_commands.len();
    for c in dual_commands {
        execute(&mut app, Action::Device(c));
    }
    println!(
        "Additional {dual_count} dual-color menu commands acknowledged, including secondary palette and RGB channels"
    );
    execute(&mut app, Action::Device(Command::Preset(preset.clone())));
    for interval in [0, 60, 120, 300] {
        app.act(Action::Interval(interval));
        assert_eq!(*app.worker.interval.lock().unwrap(), interval);
        assert_eq!(Config::load().unwrap().battery_interval, interval);
    }
    println!("All battery-check intervals saved and loaded");
    app.act(Action::Interval(0));
    for i in 0..5 {
        app.act(Action::Save(i));
        assert!(Config::load().unwrap().presets[i].is_some());
        execute(&mut app, Action::Device(Command::Polling(500)));
        let applied = execute(&mut app, Action::Load(i));
        assert_eq!(applied.polling, Some(preset.polling));
        assert_eq!(applied.idle, Some(preset.idle));
        app.act(Action::Clear(i));
        assert!(Config::load().unwrap().presets[i].is_none());
    }
    println!("All five preset slots: save, persist, apply and clear passed");
    for c in [
        Command::Polling(750),
        Command::Idle(0),
        Command::Threshold(0),
        Command::Brightness(101),
        Command::Dpi([0, 20001]),
        Command::Stages(crate::protocol::DpiStages {
            active: 0,
            values: vec![],
        }),
    ] {
        assert!(crate::worker::apply(&mouse, &c).is_err());
    }
    let mut invalid = preset.clone();
    invalid.polling = 750;
    assert!(crate::worker::apply(&mouse, &Command::Preset(invalid)).is_err());
    println!("Invalid controls and malformed preset rejected");
    app.act(Action::Startup);
    assert!(startup_enabled());
    app.act(Action::Startup);
    assert!(!startup_enabled());
    println!("Startup enable and disable passed (outer runner restores original registry value)");
    for available in [false, true] {
        for battery in [None, Some(0), Some(5), Some(50), Some(100)] {
            for charging in [false, true] {
                let icon = make_icon(battery, charging, available);
                assert!(!icon.is_null());
                unsafe {
                    DestroyIcon(icon);
                }
            }
        }
    }
    println!(
        "Battery icon handles created and freed for missing, empty, low, half, full and charging states"
    );
    app.worker.stop();
    drop(app);
    crate::worker::apply(&mouse, &Command::Preset(preset.clone())).unwrap();
    let after = mouse.snapshot();
    assert_eq!(after.polling, original.polling);
    assert_eq!(after.idle, original.idle);
    assert_eq!(after.threshold, original.threshold);
    assert_eq!(after.brightness, original.brightness);
    assert_eq!(after.dpi, original.dpi);
    assert_eq!(after.stages, original.stages);
    println!("All original readable settings restored and independently reread");
    drop(restore);
}

#[test]
#[ignore = "Starts a real tray instance and queries the mouse; requires explicit app-test authorization"]
fn native_tray_lifecycle() {
    let directory =
        std::env::temp_dir().join(format!("viper-validation-lifecycle-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    *config::TEST_DIRECTORY.lock().unwrap() = Some(directory.clone());
    Config {
        battery_interval: 0,
        ..Default::default()
    }
    .save()
    .unwrap();
    let instance = thread::spawn(super::run);
    let deadline = Instant::now() + Duration::from_secs(10);
    let hwnd = loop {
        let hwnd = unsafe { FindWindowW(wide(CLASS).as_ptr(), null()) };
        if !hwnd.is_null() {
            break hwnd;
        }
        assert!(Instant::now() < deadline, "Hidden tray window not created");
        thread::sleep(Duration::from_millis(10));
    };
    let identifier = NOTIFYICONIDENTIFIER {
        cbSize: std::mem::size_of::<NOTIFYICONIDENTIFIER>() as u32,
        hWnd: hwnd,
        uID: 1,
        ..unsafe { std::mem::zeroed() }
    };
    let wait_for_icon = || {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let mut rect = RECT::default();
            if unsafe { Shell_NotifyIconGetRect(&identifier, &mut rect) } >= 0 {
                assert!(rect.right > rect.left && rect.bottom > rect.top);
                break;
            }
            assert!(Instant::now() < deadline, "Tray icon was not registered");
            thread::sleep(Duration::from_millis(10));
        }
    };
    wait_for_icon();
    println!("Real native tray icon registered");
    let notification = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        ..unsafe { std::mem::zeroed() }
    };
    assert_ne!(unsafe { Shell_NotifyIconW(NIM_DELETE, &notification) }, 0);
    let taskbar_message = unsafe { RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()) };
    assert_ne!(unsafe { PostMessageW(hwnd, taskbar_message, 0, 0) }, 0);
    wait_for_icon();
    println!("Tray restored after simulated TaskbarCreated (Explorer itself was not restarted)");
    assert!(super::run().is_ok());
    thread::sleep(Duration::from_millis(100));
    unsafe {
        PostMessageW(hwnd, WM_CANCELMODE, 0, 0);
    }
    thread::sleep(Duration::from_millis(100));
    println!("Second instance returned successfully; popup cancelled");
    assert_ne!(unsafe { PostMessageW(hwnd, WM_CLOSE, 0, 0) }, 0);
    let deadline = Instant::now() + Duration::from_secs(10);
    while !instance.is_finished() {
        if Instant::now() >= deadline {
            unsafe {
                PostMessageW(hwnd, WM_QUIT, 0, 0);
            }
            panic!("Tray did not exit cleanly");
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(instance.join().unwrap().is_ok());
    assert!(unsafe { FindWindowW(wide(CLASS).as_ptr(), null()) }.is_null());
    let mut rect = RECT::default();
    assert!(unsafe { Shell_NotifyIconGetRect(&identifier, &mut rect) } < 0);
    *config::TEST_DIRECTORY.lock().unwrap() = None;
    std::fs::remove_dir_all(directory).unwrap();
    println!("Native close removed the window and tray icon and joined the mouse worker");
}

#[test]
#[ignore = "Restores a real mouse from VIPER_RESTORE_SNAPSHOT; requires explicit authorization"]
fn wireless_restore_saved_snapshot() {
    let path = std::env::var("VIPER_RESTORE_SNAPSHOT").expect("Snapshot path missing");
    let original: Snapshot = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let preset = Preset::capture(&original, Config::load().unwrap().lighting).unwrap();
    let mouse = Mouse::open().unwrap();
    assert_eq!(mouse.connection, "Wireless");
    crate::worker::apply(&mouse, &Command::Preset(preset)).unwrap();
    let after = mouse.snapshot();
    assert_eq!(after.polling, original.polling);
    assert_eq!(after.idle, original.idle);
    assert_eq!(after.threshold, original.threshold);
    assert_eq!(after.brightness, original.brightness);
    assert_eq!(after.dpi, original.dpi);
    assert_eq!(after.stages, original.stages);
    println!("Original wireless mouse settings restored and verified");
}
