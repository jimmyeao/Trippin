fn main() {
    // Embed logo.ico into the exe so Explorer, the taskbar and shortcuts show
    // the app icon. No-op off Windows.
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=trippin.rc");
        println!("cargo:rerun-if-changed=logo.ico");
        embed_resource::compile("trippin.rc", embed_resource::NONE)
            .manifest_optional()
            .expect("embed logo.ico");
    }
}
