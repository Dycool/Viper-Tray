use crate::{
    config::{self, Config, Lighting, Preset},
    protocol::{DpiStages, Effect, Result},
    worker::{Command, Worker},
};
use std::{
    cell::RefCell,
    ffi::OsStr,
    os::windows::ffi::OsStrExt,
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::*,
    System::{LibraryLoader::GetModuleHandleW, Registry::*, Threading::CreateMutexW},
    UI::{Shell::*, WindowsAndMessaging::*},
};

const CLASS: &str = "ViperTrayHiddenWindow";
const CALLBACK: u32 = WM_APP + 1;
const SHOW_MENU: u32 = WM_APP + 2;
fn wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(Some(0)).collect()
}
fn copy<const N: usize>(b: &mut [u16; N], s: &str) {
    let w = wide(s);
    let n = w.len().min(N - 1);
    b[..n].copy_from_slice(&w[..n]);
    b[n] = 0;
}

#[derive(Clone)]
enum Action {
    Device(Command),
    Interval(u64),
    Startup,
    Save(usize),
    Load(usize),
    Clear(usize),
    Exit,
}
struct App {
    hwnd: HWND,
    worker: Worker,
    config: Config,
    actions: Vec<Action>,
    icon: HICON,
    taskbar: u32,
    revision: u64,
    pending: bool,
    pending_lighting: Option<Option<Lighting>>,
}
impl App {
    fn new() -> Self {
        let cfg = Config::load().unwrap_or_else(|e| {
            config::log(&e);
            Config::default()
        });
        let worker = Worker::start(&cfg);
        Self {
            hwnd: null_mut(),
            worker,
            config: cfg,
            actions: vec![],
            icon: null_mut(),
            taskbar: 0,
            revision: u64::MAX,
            pending: false,
            pending_lighting: None,
        }
    }
    fn notify_data(&self) -> NOTIFYICONDATAW {
        NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd,
            uID: 1,
            ..unsafe { std::mem::zeroed() }
        }
    }
    fn add_icon(&mut self) {
        let mut n = self.notify_data();
        n.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        n.uCallbackMessage = CALLBACK;
        n.hIcon = self.icon;
        copy(&mut n.szTip, "Viper Tray — connecting");
        unsafe {
            Shell_NotifyIconW(NIM_ADD, &n);
        }
        self.revision = u64::MAX;
        self.update();
    }
    fn update(&mut self) {
        let (s, rev, busy) = {
            let st = self.worker.state.lock().unwrap();
            (st.mouse.clone(), st.revision, st.busy)
        };
        if rev == self.revision {
            return;
        }
        self.revision = rev;
        if self.pending && !busy && s.note != "Working…" {
            self.pending = false;
            if s.note == "Setting confirmed by mouse" {
                if let Some(l) = self.pending_lighting.take() {
                    self.config.lighting = l;
                    self.save_config();
                }
            } else {
                self.pending_lighting = None;
            }
        }
        let fresh = make_icon(s.battery, s.charging.unwrap_or(false), s.accessible);
        if !fresh.is_null() {
            let old = self.icon;
            self.icon = fresh;
            if !old.is_null() {
                unsafe {
                    DestroyIcon(old);
                }
            }
        }
        let mut n = self.notify_data();
        n.uFlags = NIF_ICON | NIF_TIP;
        n.hIcon = self.icon;
        let battery = s
            .battery
            .map(|b| format!("{b}%"))
            .unwrap_or_else(|| "battery unavailable".into());
        let tip = format!(
            "Viper Ultimate · {battery}{}\n{} · {}",
            if s.charging == Some(true) {
                " · charging"
            } else {
                ""
            },
            if s.connection.is_empty() {
                "Disconnected"
            } else {
                &s.connection
            },
            if busy {
                "Applying setting…"
            } else if s.accessible {
                "Right-click for settings"
            } else {
                "Connect cable to read settings"
            }
        );
        copy(&mut n.szTip, &tip);
        unsafe {
            Shell_NotifyIconW(NIM_MODIFY, &n);
        }
    }
    fn save_config(&mut self) {
        if let Err(e) = self.config.save() {
            self.record_error(&format!("Could not save app settings: {e}"));
        }
    }
    fn item(&mut self, menu: HMENU, label: &str, action: Action, enabled: bool, checked: bool) {
        self.actions.push(action);
        let id = self.actions.len();
        let flags =
            MF_STRING | if enabled { 0 } else { MF_GRAYED } | if checked { MF_CHECKED } else { 0 };
        unsafe {
            AppendMenuW(menu, flags, id, wide(label).as_ptr());
        }
    }
    fn label(&self, menu: HMENU, label: &str) {
        unsafe {
            AppendMenuW(menu, MF_STRING | MF_GRAYED, 0, wide(label).as_ptr());
        }
    }
    fn separator(&self, menu: HMENU) {
        unsafe {
            AppendMenuW(menu, MF_SEPARATOR, 0, null());
        }
    }
    fn submenu(&self, menu: HMENU, label: &str) -> HMENU {
        unsafe {
            let m = CreatePopupMenu();
            AppendMenuW(menu, MF_POPUP, m as usize, wide(label).as_ptr());
            m
        }
    }
    fn dpi_menu(
        &mut self,
        menu: HMENU,
        label: &str,
        current: [u16; 2],
        axis: Option<usize>,
        enabled: bool,
        stage: Option<(usize, &DpiStages)>,
    ) {
        let m = self.submenu(menu, label);
        let values = [
            100, 200, 400, 800, 1200, 1600, 2400, 3200, 6400, 10000, 16000, 20000,
        ];
        for value in values {
            let mut xy = current;
            if let Some(a) = axis {
                xy[a] = value
            } else {
                xy = [value; 2];
            }
            self.dpi_item(m, &value.to_string(), xy, current == xy, enabled, stage);
        }
        self.separator(m);
        for delta in [-100, -50, 50, 100] {
            let mut xy = current;
            for (a, v) in xy.iter_mut().enumerate() {
                if axis.is_none() || axis == Some(a) {
                    *v = (*v as i32 + delta).clamp(100, 20000) as u16;
                }
            }
            self.dpi_item(m, &format!("{delta:+} DPI"), xy, false, enabled, stage);
        }
    }
    fn dpi_item(
        &mut self,
        m: HMENU,
        label: &str,
        xy: [u16; 2],
        checked: bool,
        enabled: bool,
        stage: Option<(usize, &DpiStages)>,
    ) {
        let c = if let Some((i, snapshot)) = stage {
            let mut stages = snapshot.clone();
            stages.values[i] = xy;
            Command::Stages(stages)
        } else {
            Command::Dpi(xy)
        };
        self.item(m, label, Action::Device(c), enabled, checked);
    }
    fn build_menu(&mut self) -> HMENU {
        self.actions.clear();
        let (s, busy) = {
            let st = self.worker.state.lock().unwrap();
            (st.mouse.clone(), st.busy || self.pending)
        };
        let ready = s.accessible && !busy;
        let m = unsafe { CreatePopupMenu() };
        self.label(m, "Viper Ultimate");
        self.label(
            m,
            &format!(
                "{}{}",
                if s.connection.is_empty() {
                    "Disconnected"
                } else {
                    &s.connection
                },
                if busy { " · working…" } else { "" }
            ),
        );
        self.label(
            m,
            &format!(
                "Battery: {}{}",
                s.battery
                    .map(|b| format!("{b}%"))
                    .unwrap_or_else(|| "unavailable".into()),
                if s.charging == Some(true) {
                    " · charging"
                } else {
                    ""
                }
            ),
        );
        if let Some(e) = &s.error {
            let line: String = e.chars().take(85).collect();
            self.label(m, &line);
        }
        self.separator(m);
        let performance = self.submenu(m, "Performance");
        let power = self.submenu(m, "Power");
        let appearance = self.submenu(m, "Lighting");
        let p = self.submenu(
            performance,
            &format!(
                "Polling rate{}",
                s.polling.map(|v| format!(": {v} Hz")).unwrap_or_default()
            ),
        );
        for v in [125, 500, 1000] {
            self.item(
                p,
                &format!("{v} Hz"),
                Action::Device(Command::Polling(v)),
                ready,
                s.polling == Some(v),
            );
        }
        let p = self.submenu(
            power,
            &format!(
                "Idle sleep{}",
                s.idle
                    .map(|v| if v < 60 {
                        format!(": {v}s reported")
                    } else {
                        format!(": {} min", v / 60)
                    })
                    .unwrap_or_default()
            ),
        );
        for v in 1..=15 {
            self.item(
                p,
                &format!("{v} minute{}", if v == 1 { "" } else { "s" }),
                Action::Device(Command::Idle(v * 60)),
                ready,
                s.idle == Some(v * 60),
            );
        }
        let p = self.submenu(
            power,
            &format!(
                "Low-battery mode threshold{}",
                s.threshold
                    .map(|t| format!(": {}%", (t as u16 * 100 + 127) / 255))
                    .unwrap_or_default()
            ),
        );
        for v in [5, 10, 15, 20, 25] {
            let selected = s.threshold.map(|t| ((t as u16 * 100 + 127) / 255) as u8) == Some(v);
            self.item(
                p,
                &format!("{v}%"),
                Action::Device(Command::Threshold(v)),
                ready,
                selected,
            );
        }
        let p = self.submenu(
            performance,
            &format!(
                "DPI{}",
                s.dpi
                    .map(|v| format!(": {} × {}", v[0], v[1]))
                    .unwrap_or_default()
            ),
        );
        if let Some(xy) = s.dpi {
            self.dpi_menu(p, "Both axes", xy, None, ready, None);
            self.dpi_menu(p, "X axis", xy, Some(0), ready, None);
            self.dpi_menu(p, "Y axis", xy, Some(1), ready, None);
        } else {
            self.label(p, "Connect cable and refresh to read DPI");
        }
        let p = self.submenu(performance, "DPI stages");
        if let Some(stages) = &s.stages {
            for i in 0..stages.values.len() {
                let q = self.submenu(
                    p,
                    &format!(
                        "Stage {}: {} × {}",
                        i + 1,
                        stages.values[i][0],
                        stages.values[i][1]
                    ),
                );
                let mut next = stages.clone();
                next.active = (i + 1) as u8;
                self.item(
                    q,
                    "Use this stage",
                    Action::Device(Command::Stages(next)),
                    ready,
                    stages.active as usize == i + 1,
                );
                self.dpi_menu(
                    q,
                    "Set DPI",
                    stages.values[i],
                    None,
                    ready,
                    Some((i, stages)),
                );
                self.dpi_menu(
                    q,
                    "X axis",
                    stages.values[i],
                    Some(0),
                    ready,
                    Some((i, stages)),
                );
                self.dpi_menu(
                    q,
                    "Y axis",
                    stages.values[i],
                    Some(1),
                    ready,
                    Some((i, stages)),
                );
            }
            let q = self.submenu(p, "Number of stages");
            for count in 1..=5 {
                let mut next = stages.clone();
                next.values.resize(count, [1600, 1600]);
                next.active = next.active.min(count as u8);
                self.item(
                    q,
                    &count.to_string(),
                    Action::Device(Command::Stages(next)),
                    ready,
                    stages.values.len() == count,
                );
            }
        } else {
            self.label(p, "DPI stages unavailable");
        }
        let p = self.submenu(
            appearance,
            &format!(
                "Logo brightness{}",
                s.brightness
                    .map(|b| format!(": {}%", (b as u16 * 100 + 127) / 255))
                    .unwrap_or_default()
            ),
        );
        for v in [0, 10, 25, 50, 75, 100] {
            self.item(
                p,
                &format!("{v}%"),
                Action::Device(Command::Brightness(v)),
                ready,
                s.brightness.map(|b| ((b as u16 * 100 + 127) / 255) as u8) == Some(v),
            );
        }
        let lighting = self.config.lighting.clone().unwrap_or_default();
        let p = appearance;
        let effects = self.submenu(p, "Effect");
        for (label, e) in [
            ("Off", Effect::Off),
            ("Static", Effect::Static),
            ("Spectrum", Effect::Spectrum),
            ("Breathing · one color", Effect::Breathing),
            ("Breathing · two colors", Effect::BreathingDual),
            ("Breathing · random", Effect::BreathingRandom),
            ("Reactive", Effect::Reactive),
        ] {
            let mut next = lighting.clone();
            next.effect = e;
            self.item(
                effects,
                label,
                Action::Device(Command::Lighting(next)),
                ready,
                self.config.lighting.as_ref().map(|l| l.effect) == Some(e),
            );
        }
        let colors = self.submenu(p, "Colors");
        for second in [false, true] {
            let q = self.submenu(colors, if second { "Secondary" } else { "Primary" });
            for (name, c) in [
                ("Green", [0, 255, 0]),
                ("Cyan", [0, 220, 255]),
                ("Blue", [0, 80, 255]),
                ("Purple", [150, 0, 255]),
                ("Pink", [255, 0, 128]),
                ("Red", [255, 0, 0]),
                ("Orange", [255, 100, 0]),
                ("Yellow", [255, 220, 0]),
                ("White", [255, 255, 255]),
            ] {
                let mut next = lighting.clone();
                if second {
                    next.second = c
                } else {
                    next.color = c;
                }
                self.item(
                    q,
                    name,
                    Action::Device(Command::Lighting(next)),
                    ready,
                    if second {
                        lighting.second == c
                    } else {
                        lighting.color == c
                    },
                );
            }
            self.separator(q);
            let custom = self.submenu(q, "Custom RGB");
            for channel in 0..3 {
                let rgb = self.submenu(custom, ["Red", "Green", "Blue"][channel]);
                for delta in [-1i16, 1] {
                    let mut next = lighting.clone();
                    let target = if second {
                        &mut next.second[channel]
                    } else {
                        &mut next.color[channel]
                    };
                    *target = (*target as i16 + delta).clamp(0, 255) as u8;
                    self.item(
                        rgb,
                        &format!("{delta:+}"),
                        Action::Device(Command::Lighting(next)),
                        ready,
                        false,
                    );
                }
                for v in [0, 32, 64, 96, 128, 160, 192, 224, 255] {
                    let mut next = lighting.clone();
                    if second {
                        next.second[channel] = v
                    } else {
                        next.color[channel] = v;
                    }
                    self.item(
                        rgb,
                        &v.to_string(),
                        Action::Device(Command::Lighting(next)),
                        ready,
                        false,
                    );
                }
            }
        }
        let q = self.submenu(p, "Reactive duration");
        for speed in 1..=4 {
            let mut next = lighting.clone();
            next.speed = speed;
            self.item(
                q,
                &format!("{speed}"),
                Action::Device(Command::Lighting(next)),
                ready,
                lighting.speed == speed,
            );
        }
        let p = self.submenu(m, "Saved presets");
        for i in 0..5 {
            let q = self.submenu(
                p,
                &format!(
                    "Preset {}{}",
                    i + 1,
                    if self.config.presets[i].is_some() {
                        ""
                    } else {
                        " · empty"
                    }
                ),
            );
            self.item(
                q,
                "Apply",
                Action::Load(i),
                ready && self.config.presets[i].is_some(),
                false,
            );
            self.item(
                q,
                "Save displayed mouse settings",
                Action::Save(i),
                ready,
                false,
            );
            self.item(
                q,
                "Clear",
                Action::Clear(i),
                self.config.presets[i].is_some(),
                false,
            );
        }
        self.separator(m);
        let app_menu = self.submenu(m, "Settings");
        self.item(
            app_menu,
            "Refresh mouse settings",
            Action::Device(Command::Refresh),
            !busy,
            false,
        );
        let p = self.submenu(app_menu, "Battery checks");
        for (v, label) in [
            (0, "Manual only"),
            (60, "Every minute"),
            (120, "Every 2 minutes"),
            (300, "Every 5 minutes"),
        ] {
            self.item(
                p,
                label,
                Action::Interval(v),
                true,
                self.config.battery_interval == v,
            );
        }
        self.item(
            app_menu,
            "Start with Windows",
            Action::Startup,
            true,
            startup_enabled(),
        );
        self.separator(m);
        self.item(m, "Exit", Action::Exit, true, false);
        m
    }
    fn menu(&mut self) {
        let m = self.build_menu();
        let mut point = POINT::default();
        unsafe {
            GetCursorPos(&mut point);
            SetForegroundWindow(self.hwnd);
            let id = TrackPopupMenu(
                m,
                TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
                point.x,
                point.y,
                0,
                self.hwnd,
                null(),
            );
            PostMessageW(self.hwnd, WM_NULL, 0, 0);
            DestroyMenu(m);
            if id > 0
                && let Some(action) = self.actions.get(id as usize - 1).cloned()
            {
                self.act(action);
            }
        }
    }
    fn act(&mut self, a: Action) {
        match a {
            Action::Device(c) => {
                self.pending_lighting = if let Command::Lighting(l) = &c {
                    Some(Some(l.clone()))
                } else {
                    None
                };
                self.queue(c);
            }
            Action::Interval(v) => {
                self.config.battery_interval = v;
                *self.worker.interval.lock().unwrap() = v;
                self.save_config();
            }
            Action::Startup => {
                if let Err(e) = set_startup(!startup_enabled()) {
                    self.record_error(&e);
                }
            }
            Action::Save(i) => {
                let snap = self.worker.state.lock().unwrap().mouse.clone();
                match Preset::capture(&snap, self.config.lighting.clone()) {
                    Ok(p) => {
                        self.config.presets[i] = Some(p);
                        self.save_config();
                    }
                    Err(e) => self.record_error(&e),
                }
            }
            Action::Load(i) => {
                if let Some(p) = self.config.presets[i].clone() {
                    self.pending_lighting = Some(p.lighting.clone());
                    self.queue(Command::Preset(p));
                }
            }
            Action::Clear(i) => {
                self.config.presets[i] = None;
                self.save_config();
            }
            Action::Exit => unsafe {
                PostMessageW(self.hwnd, WM_CLOSE, 0, 0);
            },
        }
    }
    fn record_error(&mut self, error: &str) {
        config::log(error);
        let mut st = self.worker.state.lock().unwrap();
        st.mouse.error = Some(error.into());
        st.revision += 1;
    }
    fn queue(&mut self, c: Command) {
        self.pending = true;
        {
            let mut st = self.worker.state.lock().unwrap();
            st.busy = true;
            st.mouse.note = "Working…".into();
            st.revision += 1;
        }
        if self.worker.tx.send(c).is_err() {
            self.pending = false;
            self.record_error("Mouse worker stopped. Restart Viper Tray.");
        }
    }
}

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
fn startup_enabled() -> bool {
    unsafe {
        let mut key = null_mut();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            wide(RUN_KEY).as_ptr(),
            0,
            KEY_READ,
            &mut key,
        ) != 0
        {
            return false;
        }
        let found = RegQueryValueExW(
            key,
            wide("ViperTray").as_ptr(),
            null(),
            null_mut(),
            null_mut(),
            null_mut(),
        ) == 0;
        RegCloseKey(key);
        found
    }
}
fn set_startup(enabled: bool) -> Result<()> {
    let command = if enabled {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        Some(wide(&format!("\"{}\"", exe.display())))
    } else {
        None
    };
    unsafe {
        let mut key = null_mut();
        if RegCreateKeyExW(
            HKEY_CURRENT_USER,
            wide(RUN_KEY).as_ptr(),
            0,
            null(),
            0,
            KEY_SET_VALUE,
            null(),
            &mut key,
            null_mut(),
        ) != 0
        {
            return Err("Could not open Windows startup settings".into());
        }
        let result = if let Some(s) = command {
            RegSetValueExW(
                key,
                wide("ViperTray").as_ptr(),
                0,
                REG_SZ,
                s.as_ptr().cast(),
                (s.len() * 2) as u32,
            )
        } else {
            RegDeleteValueW(key, wide("ViperTray").as_ptr())
        };
        RegCloseKey(key);
        if result != 0 {
            return Err(format!("Could not update Windows startup ({result})"));
        }
        Ok(())
    }
}

// Original battery artwork from Tekk-Know/RazerBatteryTaskbar; see assets/battery/README.md.
const BATTERY_PIXELS: [&[u8; 4096]; 11] = [
    include_bytes!("../assets/battery/battery_0.bgra"),
    include_bytes!("../assets/battery/battery_10.bgra"),
    include_bytes!("../assets/battery/battery_20.bgra"),
    include_bytes!("../assets/battery/battery_30.bgra"),
    include_bytes!("../assets/battery/battery_40.bgra"),
    include_bytes!("../assets/battery/battery_50.bgra"),
    include_bytes!("../assets/battery/battery_60.bgra"),
    include_bytes!("../assets/battery/battery_70.bgra"),
    include_bytes!("../assets/battery/battery_80.bgra"),
    include_bytes!("../assets/battery/battery_90.bgra"),
    include_bytes!("../assets/battery/battery_100.bgra"),
];
fn make_icon(battery: Option<u8>, _charging: bool, available: bool) -> HICON {
    let level = if available {
        battery.unwrap_or(0).min(100) / 10
    } else {
        0
    };
    let pixels = BATTERY_PIXELS[level as usize];
    let mut mask = [0xffu8; 128];
    for y in 0..32 {
        for x in 0..32 {
            if pixels[(y * 32 + x) * 4 + 3] != 0 {
                mask[y * 4 + x / 8] &= !(0x80 >> (x % 8));
            }
        }
    }
    unsafe {
        CreateIcon(
            GetModuleHandleW(null()),
            32,
            32,
            1,
            32,
            mask.as_ptr(),
            pixels.as_ptr(),
        )
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if msg == WM_NCCREATE {
        let cs = unsafe { &*(l as *const CREATESTRUCTW) };
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
        }
    }
    let p = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut RefCell<App>;
    if p.is_null() {
        return unsafe { DefWindowProcW(hwnd, msg, w, l) };
    }
    let cell = unsafe { &*p };
    // Native popup menus pump messages. RefCell prevents overlapping mutable
    // app references when timer or shell callbacks arrive while a menu is open.
    let Ok(mut app) = cell.try_borrow_mut() else {
        // Popup tracking reenters the owner window for native menu painting.
        // Let Windows process those messages without borrowing App again.
        if matches!(
            msg,
            WM_TIMER | crate::worker::STATE_CHANGED | CALLBACK | SHOW_MENU
        ) {
            return 0;
        }
        return unsafe { DefWindowProcW(hwnd, msg, w, l) };
    };
    if msg == app.taskbar && app.taskbar != 0 {
        app.add_icon();
        return 0;
    }
    match msg {
        WM_CREATE => {
            app.hwnd = hwnd;
            app.worker.set_window(hwnd);
            app.taskbar = unsafe { RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()) };
            app.icon = make_icon(None, false, false);
            app.add_icon();
            unsafe {
                SetTimer(hwnd, 1, 1000, None);
            }
            0
        }
        WM_TIMER | crate::worker::STATE_CHANGED => {
            app.update();
            0
        }
        CALLBACK => {
            let event = l as u32;
            if event == WM_RBUTTONUP || event == WM_LBUTTONUP || event == WM_CONTEXTMENU {
                app.menu();
            }
            0
        }
        SHOW_MENU => {
            app.menu();
            0
        }
        WM_CLOSE => {
            drop(app);
            unsafe {
                DestroyWindow(hwnd);
            }
            0
        }
        WM_DESTROY => {
            unsafe {
                KillTimer(hwnd, 1);
                Shell_NotifyIconW(NIM_DELETE, &app.notify_data());
                if !app.icon.is_null() {
                    DestroyIcon(app.icon);
                }
                PostQuitMessage(0);
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, w, l) },
    }
}

pub fn run() -> Result<()> {
    unsafe {
        let mutex = CreateMutexW(null(), 0, wide("Local\\ViperTray.SingleInstance").as_ptr());
        if mutex.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let existing = FindWindowW(wide(CLASS).as_ptr(), null());
            if !existing.is_null() {
                PostMessageW(existing, SHOW_MENU, 0, 0);
            }
            CloseHandle(mutex);
            return Ok(());
        }
        let instance = GetModuleHandleW(null());
        let class = wide(CLASS);
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            lpszClassName: class.as_ptr(),
            ..std::mem::zeroed()
        };
        if RegisterClassW(&wc) == 0 {
            CloseHandle(mutex);
            return Err("Could not register tray window".into());
        }
        let app = Box::new(RefCell::new(App::new()));
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            wide("Viper Tray").as_ptr(),
            0,
            0,
            0,
            0,
            0,
            null_mut(),
            null_mut(),
            instance,
            (&*app as *const RefCell<App>).cast_mut().cast(),
        );
        if hwnd.is_null() {
            CloseHandle(mutex);
            return Err("Could not create hidden tray window".into());
        }
        let mut msg: MSG = std::mem::zeroed();
        loop {
            let r = GetMessageW(&mut msg, null_mut(), 0, 0);
            if r <= 0 {
                break;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        app.borrow_mut().worker.stop();
        CloseHandle(mutex);
        Ok(())
    }
}

#[cfg(test)]
#[path = "app_validation.rs"]
mod validation_tests;
