# Validation

Tested on Windows with a wireless Razer Viper Ultimate, firmware 1.7, on 2026-10-08.

## Passed

| Feature | Evidence |
| --- | --- |
| Polling | All three menu rates: 125, 500, 1,000 Hz, confirmed by readback |
| Idle sleep | Every duration from 1 through 15 minutes, confirmed by readback |
| Low-battery threshold | All five menu thresholds, confirmed by raw-value readback |
| DPI | All menu values and increments for both axes, X alone, and Y alone |
| DPI stages | All five stages, independent axes, active-stage selection, and stage counts 1–5 |
| Brightness | All six menu percentages, confirmed by readback |
| Lighting | All seven effects, palettes, RGB channels/increments, and duration values acknowledged by the device |
| Two-color lighting | Rebuilt the menu in dual-color mode; 89 additional palette/channel/duration commands acknowledged |
| Presets | Every one of five slots saved, persisted, applied, and cleared; original settings restored |
| Battery checks | All four intervals persisted and loaded; input-break/sleep deferral and bounded backoff tested |
| Battery/charging/firmware | Read through the wireless receiver; icon handles created/freed across unavailable, empty, low, half, full, and charging states |
| Startup | Actual per-user startup entry enabled and disabled; runner restored the exact original entry |
| Menu construction | 501 nonempty native menu labels; nested structure contains no diagnostics, settings-age line, or forced-left flags |
| Tray lifecycle | Actual native icon registered; icon deletion plus TaskbarCreated restored it; second instance returned; close removed the icon/window and joined the worker |
| Invalid input | Invalid polling, sleep, threshold, brightness, DPI, stages, and malformed preset rejected |
| Timeout/cancellation | A real pending Windows named-pipe operation timed out, cancelled, and released its request buffers |
| Layout | Native Windows positioning; custom submenu spacing and forced-left flags removed |
| Notifications | No balloon, toast, or message-box notification path in application source |

The complete wireless control suite passed **423 initial menu commands plus 89 dual-color commands: 512 menu-generated hardware actions**. Preset, restoration, and error-validation checks ran in addition. All original readable mouse values were independently reread after restoration: 1,000 Hz, 900-second sleep, raw threshold 13, brightness 0, 3,200 × 3,200 DPI, and the original five stages with stage 5 active. User preferences/presets were isolated from test writes.

**Twelve ordinary software tests pass for v1.0.0, including repeated Windows log rotation.** Four integration/recovery tests are ignored by default. Formatter, strict Clippy, and release build are checked separately. Hardware tests are never enabled in GitHub Actions.

## Failure found and fixed

The first full run passed the mouse controls but stalled during HID enumeration in the preset phase. Tracing showed it stopped before sending a mouse request. The app previously enumerated and queried all HID devices through a third-party backend; it now filters the Windows interface list to the Viper control interface before opening devices, and performs cancellable overlapped feature I/O. The full hardware and tray suites passed after that change.

An unchanged wireless polling write plus verified readback took **32, 30, and 33 ms** in three samples with the new transport. These samples are not a guarantee for every command or receiver condition.

## Limits of this validation

Lighting has an acknowledgement but no effect readback: physical color, timing, and appearance were not visually observed. Charging transitions, a full idle-sleep/wake cycle, sustained movement without freezes, unplugging while an I/O request is pending, actual Explorer process restart, and visual menu overlap at different desktop scale settings remain physical/integration checks. Taskbar recovery was simulated through the real application handler without restarting Explorer. Configured polling readback does not measure delivered input-event frequency.

## Reproduce

```powershell
cargo fmt --check
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
```

With explicit authorization to temporarily change an awake wireless mouse:

```powershell
./scripts/test-hardware.ps1
```

## v1.0.0 release review

The full wireless menu-control suite was rerun after the release fixes: all 512 commands, five presets, restoration, startup toggling, native tray recovery/exit, and unchanged-polling latency passed. The three latency samples were 33, 32, and 32 ms. Original readable mouse settings and the startup entry were restored; test preferences stayed isolated.

DPI stage menus now use the same frozen snapshot for every entry, preventing a disconnect during menu construction from invalidating stage indexing. Preset completion rereads both DPI and stages. Repeated log rotation replaces the previous file on Windows, and preference-save failures appear in the menu. Menu placement is left entirely to Windows.

The tag workflow runs formatting, unit tests and strict Clippy, checks the Cargo/tag version, builds Windows downloads, compares every uploaded asset with the local SHA-256 hash, and only then publishes the release.
