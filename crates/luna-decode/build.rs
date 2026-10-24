fn main() {
    println!("cargo:rerun-if-changed=shim/luna_avf.m");
    // The AVFoundation shim is macOS only (src/platform_macos.rs).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new()
            .file("shim/luna_avf.m")
            .flag("-fobjc-arc")
            .compile("luna_avf");
        for framework in [
            "AVFoundation",
            "CoreMedia",
            "CoreVideo",
            "Foundation",
            "VideoToolbox",
        ] {
            println!("cargo:rustc-link-lib=framework={framework}");
        }
    }
}
