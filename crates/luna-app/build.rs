fn main() {
    // Luna draws its own controls; the std-widgets it does use (ScrollView,
    // AboutSlint) take the dark Fluent look so they sit on the dark surfaces.
    let config = slint_build::CompilerConfiguration::new().with_style("fluent-dark".into());
    slint_build::compile_with_config("ui/app.slint", config).expect("Slint UI should compile");
}
