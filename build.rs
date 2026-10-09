fn main() {
    println!("cargo:rerun-if-changed=assets/viper-ultimate.ico");
    for file in [
        "LICENSE",
        "THIRD_PARTY_NOTICES.md",
        "README.md",
        "VALIDATION.md",
    ] {
        println!("cargo:rerun-if-changed={file}");
    }
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource
            .set_icon("assets/viper-ultimate.ico")
            .set("ProductName", "Viper-Tray")
            .set("FileDescription", "Viper-Tray mouse controls")
            .set("OriginalFilename", "Viper-tray.exe")
            .append_rc_content(
                "101 RCDATA \"LICENSE\"\n102 RCDATA \"THIRD_PARTY_NOTICES.md\"\n103 RCDATA \"README.md\"\n104 RCDATA \"VALIDATION.md\"",
            )
            .compile()
            .expect("Could not embed the executable icon and documentation");
    }
}
