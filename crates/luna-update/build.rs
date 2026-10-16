fn main() {
    println!("cargo:rerun-if-changed=shim/luna_sparkle.m");
    // The Sparkle shim is macOS only; it links Foundation, not Sparkle.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new()
            .file("shim/luna_sparkle.m")
            .flag("-fobjc-arc")
            .compile("luna_sparkle");
        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rustc-link-lib=objc");
    }
}
