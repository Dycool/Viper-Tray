# Validation status

Testing resumed with the user's authorization on 2026-10-08, with the mouse connected wirelessly.

Verified:

- All seven unit tests pass on Windows: packet framing, response validation, failure/busy handling, independent-axis DPI stages, invalid DPI/stage rejection, lighting payloads, incomplete preset rejection, and bounded timeout backoff (some checks share a test).
- Formatter and Clippy checks pass; the release executable builds successfully.
- The Rust diagnostic command successfully reads the wireless receiver: firmware 1.7, battery 100%, not charging, 1,000 Hz polling, 900-second idle sleep, raw low-battery threshold 13, brightness 0, and DPI 3,200 on both axes.
- Five DPI stages read back as 400, 800, 1,600, 2,400 and 3,200 on both axes, with stage 5 active.
- The tray process starts and remains running. No diagnostic errors were logged at the initial observation.

Interactive testing is in progress. The user reported that the tray menu opened with blank labels. The menu owner now forwards reentrant Windows painting messages to the default handler instead of swallowing them. The app was rebuilt and restarted with the original RazerBatteryTaskbar battery icon artwork. The user subsequently reported that it seems to be working fine. This is a user-observed smoke test; systematic setting write/readback and prolonged freeze testing remain pending. The settings-read age line was removed from the menu at the user's request.

Pending:

1. Confirm setting write/readback and mouse movement during manual settings changes and background battery checks.
2. Verify brightness/lighting, independent DPI axes, stage editing, and preset save/apply while preserving the user's original settings.
3. Check reconnect, timeout recovery, second-instance behavior, Explorer restart recovery, and clean exit.
4. Verify startup enable/disable changes only the app's own per-user entry.
5. Verify icon legibility, checkmarks, error/update messages, and manual-only battery mode.

The wireless readback confirms the configured polling rate; it does not measure USB event frequency or prove that movement is free of freezes.
