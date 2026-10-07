# Third-party notices

## OpenRazer

The device report layout, checksum, transaction identifiers, validation, DPI encoding and lighting command formats were derived from:

- https://github.com/openrazer/openrazer/blob/master/driver/razercommon.c
- https://github.com/openrazer/openrazer/blob/master/driver/razercommon.h
- https://github.com/openrazer/openrazer/blob/master/driver/razerchromacommon.c
- https://github.com/openrazer/openrazer/blob/master/driver/razermouse_driver.c
- https://github.com/openrazer/openrazer/blob/master/daemon/openrazer_daemon/hardware/mouse.py

These upstream driver files identify their license as GPL-2.0-or-later. Copyright notices in the sources include Tim Theede, Terri Cain and other OpenRazer contributors. This application's protocol implementation and application are distributed under GPL-2.0-or-later. The Windows HID transport and native menu implementation are newly written Rust code.

## Rust dependencies

Direct dependencies: `hidapi` (MIT; native backend), `windows-sys` (MIT OR Apache-2.0), `serde` (MIT OR Apache-2.0), and `serde_json` (MIT OR Apache-2.0). Exact versions and transitive dependencies are in Cargo.lock. Dependency license metadata and notices are in their published source packages. The release source is supplied alongside the local executable.

The Corsair Elite Display and RazerBatteryTaskbar repositories are interaction/design references only. Their application code is not bundled or copied into this repository.
