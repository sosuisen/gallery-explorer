//! Embeds the app icon in the exe. Explorer, the desktop and the taskbar show this icon.

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    // The build script runs on the host, so `cfg(windows)` would be wrong in a cross build.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // The icon gets the resource ID 1. src/infra/app_icon.rs loads it by this ID.
        winresource::WindowsResource::new()
            .set_icon("assets/icon.ico")
            .compile()
            .expect("the icon could not be embedded in the exe");
    }
}
