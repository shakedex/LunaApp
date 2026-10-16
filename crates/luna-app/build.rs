fn main() {
    // Luna draws its own controls; the std-widgets it does use (ScrollView,
    // AboutSlint) take the dark Fluent look so they sit on the dark surfaces.
    let config = slint_build::CompilerConfiguration::new().with_style("fluent-dark".into());
    slint_build::compile_with_config("ui/app.slint", config).expect("Slint UI should compile");

    // Luna.exe's icon and version, which Explorer, the taskbar and the
    // WinSparkle update window show.
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=../../Assets/luna-logo.ico");
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../Assets/luna-logo.ico")
            .set("ProductName", "Luna")
            .set("FileDescription", "Luna")
            .set("CompanyName", "Shaked Lipszyc");
        res.compile().expect("Windows resources should compile");
    }
}
