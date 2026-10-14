fn main() {
    println!("cargo:rerun-if-changed=assets/icon.rc");
    println!("cargo:rerun-if-changed=assets/icon.ico");
    // GPUI loads the window icon from icon resource 1 of the exe.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("assets/icon.rc", embed_resource::NONE)
            .manifest_optional()
            .unwrap();
    }
}
