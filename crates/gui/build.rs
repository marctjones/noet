fn main() {
    // DragArea/DropArea (kanban drag-and-drop) are behind Slint's experimental flag.
    std::env::set_var("SLINT_ENABLE_EXPERIMENTAL_FEATURES", "1");
    // Use the Fluent style: Windows 11's design language, for a native Win11
    // look that renders identically (and lightly) on both Ubuntu and Windows.
    let config = slint_build::CompilerConfiguration::new().with_style("fluent".into());
    slint_build::compile_with_config("ui/app.slint", config).unwrap();
}
