#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod app;
mod config;
mod device;
mod protocol;
mod worker;
fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--diagnose") {
        let result = device::Mouse::open()
            .map(|m| m.snapshot())
            .unwrap_or_else(|e| device::Snapshot {
                error: Some(e),
                ..Default::default()
            });
        let out = serde_json::to_string_pretty(&result).unwrap();
        if let Some(i) = args.iter().position(|a| a == "--output") {
            if let Some(p) = args.get(i + 1) {
                std::fs::write(p, &out).expect("Write diagnostics");
            }
        } else {
            println!("{out}");
        }
        std::process::exit(if result.accessible { 0 } else { 1 });
    }
    if let Err(e) = app::run() {
        config::log(&e);
    }
}
