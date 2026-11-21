//! Embed the shared application icon in Windows executable builds.
fn main() {
    println!("cargo:rerun-if-changed=assets/logo.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("assets/logo.ico")
            .compile()
            .expect("embed Windows icon");
    }
}
