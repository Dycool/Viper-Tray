# Validation status

Testing resumed with the user's authorization on 2026-10-08, with the mouse connected wirelessly.

Verified:

- All eight unit tests pass on Windows: packet framing, response validation, failure/busy handling, independent-axis DPI stages, invalid DPI/stage rejection, lighting payloads, incomplete preset rejection, bounded timeout backoff, and preservation of unrelated cached settings after individual changes (some checks share a test).
- Formatter and Clippy checks pass; the release executable builds successfully.
- The Rust diagnostic command successfully reads the wireless receiver: firmware 1.7, battery 100%, not charging, 1,000 Hz polling, 900-second idle sleep, raw low-battery threshold 13, brightness 0, and DPI 3,200 on both axes.
- Five DPI stages read back as 400, 800, 1,600, 2,400 and 3,200 on both axes, with stage 5 active.
- The tray process starts and remains running. No diagnostic errors were logged at the initial observation.

Interactive testing is in progress. The user reported that the tray menu opened with blank labels. The menu owner now forwards reentrant Windows painting messages to the default handler instead of swallowing them. The app was rebuilt and restarted with the original RazerBatteryTaskbar battery icon artwork. The user subsequently reported that it seems to be working fine. This is a user-observed smoke test; systematic testing of all settings and prolonged freeze testing remain pending. The settings-read age line was removed from the menu at the user's request.

Responsiveness and notification update:

- Removed every balloon notification path, including success, errors, presets, startup, and help fallback. Errors remain in the menu and log.
- Ordinary settings changes now keep unrelated cached values instead of performing a full nine-command snapshot. Setters still verify their own readback; DPI and stage changes reread their coupled values.
- Worker completion posts directly to the tray window instead of waiting for the one-second timer. The timer remains as a fallback during native popup tracking.
- The first acknowledgement read waits 10 ms; a busy response retains the 80 ms retry delay and bounded retries.
- An explicitly invoked wireless hardware test reapplied the existing polling rate three times and verified readback. Completion took 246, 230 and 225 ms, including device enumeration and write/readback. The earlier optimized path with the old 80 ms initial pause took 375, 368 and 361 ms. These are three observed samples, not a guarantee for every setting or wireless condition.
- Eight ordinary tests pass. The hardware test is ignored by default and only runs when explicitly requested. Formatter, strict Clippy and release build pass.

Pending:

1. Confirm remaining setting write/readback and mouse movement during manual settings changes and background battery checks.
2. Verify brightness/lighting, independent DPI axes, stage editing, and preset save/apply while preserving the user's original settings.
3. Check reconnect, timeout recovery, second-instance behavior, Explorer restart recovery, and clean exit.
4. Verify startup enable/disable changes only the app's own per-user entry.
5. Verify icon legibility, checkmarks, error/update messages, and manual-only battery mode.

The wireless readback confirms the configured polling rate; it does not measure USB event frequency or prove that movement is free of freezes.
