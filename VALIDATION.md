# Validation status

Further tests and hardware access were paused at the user's request. Do not launch the app or run device queries until the user authorizes testing.

Completed before pause:

- Seven initial tests passed: report framing, response validation, failure/busy handling, independent-axis stage roundtrip, invalid DPI/stage rejection, lighting payloads, incomplete preset rejection, and timeout backoff (some checks share a test).
- The initial Rust code compiled.

Changes after pause are compiled only; tests have not been rerun.

Pending when authorized:

1. Run formatter check, unit tests and Clippy on the final source.
2. Read wired device state through the Rust diagnostic command; confirm battery, firmware, polling, idle, DPI/stages and brightness independently.
3. Snapshot original values, write unchanged settings and validate readback; verify lighting acknowledgements, then restore the original appearance where its original effect is known.
4. Verify wireless reads and timeout/backoff behavior with the actual receiver. Check for movement freezes with battery checks enabled and manual-only mode.
5. Launch the tray; check right-click menus, checkmarks, update/error tooltips, icon legibility, preset save/apply, reconnect, second-instance behavior, Explorer restart recovery and clean exit.
6. Enable/disable startup and confirm only the app's own per-user entry is changed.
7. Confirm presets preserve raw brightness/threshold values and reject malformed imported configuration before any writes.
