fn main() {
    // Embed logo.ico into the exe so Explorer, the taskbar and shortcuts show
    // the app icon. Windows only — macOS takes its icon from the .app bundle's
    // .icns. The cfg covers the host; the env check covers the target when
    // cross-compiling.
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=trippin.rc");
        println!("cargo:rerun-if-changed=logo.ico");
        embed_resource::compile("trippin.rc", embed_resource::NONE)
            .manifest_optional()
            .expect("embed logo.ico");
    }
}
