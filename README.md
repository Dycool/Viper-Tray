# Viper Tray

A small **native Rust Windows notification-area app** for the Razer Viper Ultimate. Right-click the battery icon to control the mouse. There is no main window, settings page, webview, Electron runtime, Synapse dependency, or custom driver.

## Use

Run `viper-tray.exe`, then right-click its battery icon beside the Windows clock. Windows may place it in the tray overflow initially. Hover for battery percentage, charging status, and connection information.

The app reads the device on connection and displays the last reported settings with their age. Use **Refresh mouse settings** after changing settings elsewhere or pressing the mouse's DPI button. Mouse settings are never automatically overwritten at startup or reconnect.

The wired connection is preferred when both the cable and receiver are attached. If the wireless receiver cannot reach the mouse, controls are unavailable and the menu explains that a cable is needed. Cached values are not treated as live confirmation. Closing competing mouse control software can help avoid contention.

## Tray controls

| Menu | Implementation |
| --- | --- |
| Battery and charging | Battery-shaped icon, percentage tooltip, charging indicator |
| Polling rate | 125, 500, and 1,000 Hz |
| Idle sleep | 1–15 minutes; no unsupported “disable sleep” value |
| Low-battery threshold | 5%, 10%, 15%, 20%, 25% |
| DPI | 100–20,000; common values, ±50/100 increments, separate X/Y axes |
| DPI stages | 1–5 stages, edit each stage, choose active stage |
| Logo brightness | 0%, 10%, 25%, 50%, 75%, 100% |
| Logo lighting | Off, static, spectrum, reactive, single/dual/random breathing |
| Colors | Palette plus separate RGB channel menus |
| Saved presets | Five local slots; save displayed settings, apply, clear |
| Battery checks | Manual, 60, 120, or 300 seconds |
| Start with Windows | Optional per-user startup entry; off until enabled |
| Diagnostics | Firmware, errors, local log |

Presets are app-managed settings stored on the PC, **not Synapse's five onboard profile slots**. DPI stages and other settings use the documented device storage commands. Refresh before saving a preset if the displayed settings are old. Lighting selections are the app's last acknowledged selections; OpenRazer does not expose an effect readback for this mouse, so the app cannot identify a pre-existing effect selected by another application. Scalar settings and DPI stages are verified by readback. Lighting requires an explicit successful acknowledgement.

## Synapse parity and limits

This is a replacement for the **OpenRazer-supported Viper Ultimate controls**, not complete Synapse parity. These functions are not implemented:

- Hardware button reassignment, Hypershift, keyboard macros, application-linked profiles, and onboard profile-slot management.
- Surface calibration, lift-off/landing distance, asymmetric cut-off, and smart tracking.
- Chroma Studio, screen/audio lighting synchronization, and charging dock controls.
- Firmware updates, pairing/re-pairing, or memory-repair commands.

OpenRazer's Viper Ultimate capability list does not expose most of those device commands. Global Windows hooks would affect other pointing devices and would not provide hardware remapping; this app does not silently substitute them. Future protocol additions belong in `device.rs`, with validation in `protocol.rs`, and can be exposed through the same native menus.

## Avoiding periodic freezes

All USB requests run on one worker thread, never on the menu thread. HID handles are released between operations. There is no overlapping battery polling, continuous device reset, or setting reapplication loop.

Automatic battery checks are deferred while the user is actively providing input, and while idle time suggests the mouse is asleep. A failed check backs off exponentially, up to 15 minutes. Full settings are refreshed on connection or explicit request, not on every battery check. Manual-only mode makes no periodic battery requests; initial connection detection still reads settings once.

This does not guarantee that a firmware/receiver will accept wireless commands without stutter. In the development session the receiver returned timeouts while wired commands worked. **Live verification of this Rust application has been postponed at the owner's request.**

## Portable build

Windows 10/11 x64, stable Rust (2024 edition), and MSVC build tools are required.

```powershell
cargo build --release --locked
```

The executable is `target\release\viper-tray.exe`. A prepared portable folder is placed in `dist\ViperTray` during local packaging. Keep its README next to the executable for the tray's help action.

For maintainers, when testing is authorized:

```powershell
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
target\release\viper-tray.exe --diagnose --output mouse-diagnostics.json
```

`--diagnose` performs read-only mouse queries and exits without creating a tray icon. It does not modify settings. It returns a failing exit code if the primary settings query cannot reach the mouse. Individual unsupported/failed fields remain unavailable and are reported in the JSON.

## Settings and privacy

App preferences, presets and a rotating diagnostic log live in `%APPDATA%\ViperTray`. There is no network access, telemetry, cloud account, or background service. The app connects only to USB vendor `1532`, product `007A` (wired) or `007B` (receiver), interface 0 / mouse usage. Multiple matching devices of the same connection type are rejected rather than picking one arbitrarily.

The optional startup entry is `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\ViperTray`. Disable **Start with Windows**, exit the app, and delete the portable folder to uninstall. Delete `%APPDATA%\ViperTray` if you also want to remove presets and logs.

Applying a preset is a sequence of acknowledged writes. If a later write fails, earlier changes may already have reached the mouse; the app stops and reports the failure. It does not claim a transaction or roll back to guessed values.

## Development status

The initial seven protocol/configuration tests passed before the owner paused further testing. Subsequent code edits and the executable are build-checked only. No Rust-app mouse queries, live settings changes, tray launch, or interactive menu tests have been performed after that pause. See `VALIDATION.md` for the pending checklist.

## Credits and license

The Razer protocol is derived from [OpenRazer](https://github.com/openrazer/openrazer). Tray interaction follows the owner's [Corsair Elite Display](https://github.com/Dycool/corsair-elite-display) reference; no Corsair code is copied. Battery tray behavior was inspired by [RazerBatteryTaskbar](https://github.com/Tekk-Know/RazerBatteryTaskbar).

GPL-2.0-or-later. See `LICENSE` and `THIRD_PARTY_NOTICES.md`.
