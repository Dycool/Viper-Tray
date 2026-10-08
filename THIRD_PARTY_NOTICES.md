# Third-party notices

## OpenRazer

The device report layout, checksum, transaction identifiers, validation, DPI encoding and lighting command formats were derived from:

- https://github.com/openrazer/openrazer/blob/master/driver/razercommon.c
- https://github.com/openrazer/openrazer/blob/master/driver/razercommon.h
- https://github.com/openrazer/openrazer/blob/master/driver/razerchromacommon.c
- https://github.com/openrazer/openrazer/blob/master/driver/razermouse_driver.c
- https://github.com/openrazer/openrazer/blob/master/daemon/openrazer_daemon/hardware/mouse.py

These upstream driver files identify their license as GPL-2.0-or-later. Copyright notices in the sources include Tim Theede, Terri Cain and other OpenRazer contributors. This application's protocol implementation and application are distributed under GPL-2.0-or-later. The Windows HID transport and native menu implementation are newly written Rust code. Device discovery uses the Windows Configuration Manager interface list, filtered before opening the Viper control interface; feature I/O uses overlapped requests and cancellation.

## Rust dependencies

Direct dependencies: `windows-sys` (MIT OR Apache-2.0), `serde` (MIT OR Apache-2.0), and `serde_json` (MIT OR Apache-2.0). Exact versions and transitive dependencies are in Cargo.lock. Dependency license metadata and notices are in their published source packages. The release source is supplied alongside the local executable.

RazerBatteryTaskbar is an interaction/design reference. Its application code is not bundled or copied into this repository.

## Battery artwork

The battery PNG assets and derived BGRA data in `assets/battery` originate from [Tekk-Know/RazerBatteryTaskbar](https://github.com/Tekk-Know/RazerBatteryTaskbar). See `assets/battery/README.md` for attribution and the upstream license status.
