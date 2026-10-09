## Viper-Tray v1.0.0

First stable release of the native Windows tray app for the Razer Viper Ultimate.

- Battery percentage and charging status in the tray tooltip.
- Polling rate, idle sleep, low-battery threshold, DPI and up to five DPI stages.
- Logo brightness and lighting effects, including custom RGB colors.
- Five saved local presets and optional startup with Windows.
- Native right-click menus positioned by Windows, with no notifications or main window.
- Viper-only discovery, verified settings readback, and cancellable requests with a timeout.

### Download

Download **ViperTray-windows-x64.zip**, extract it, and run **Viper-tray.exe**. Windows 10/11 x64 is required; Synapse and a custom driver are not required. The ZIP contains only `Viper-tray.exe`, with its mouse icon, tray artwork, and notices embedded. No external assets are required.

### Verification and limits

Software checks and the wireless menu-control and tray lifecycle suites are documented in VALIDATION.md. Lighting acknowledgements do not verify physical appearance, and configured polling readback does not measure delivered event frequency. Long sleep/wake, charging transitions, and sustained freeze-free movement still need physical observation.

This release supports the Viper Ultimate only. Button remapping, macros, Hypershift, surface calibration, dock controls, pairing, firmware updates, and Synapse onboard profile management are not implemented. Presets are stored on the PC; failed multi-setting writes may partially apply.
