fn main() {
    // DragArea/DropArea (kanban drag-and-drop) are behind Slint's experimental flag.
    std::env::set_var("SLINT_ENABLE_EXPERIMENTAL_FEATURES", "1");
    // Use the Fluent style: Windows 11's design language, for a native Win11
    // look that renders identically (and lightly) on both Ubuntu and Windows.
    let mut config = slint_build::CompilerConfiguration::new().with_style("fluent".into());
    // Debug builds emit Slint debug info so the headless GUI tests can use the
    // ElementHandle / ElementQuery API (find elements, simulate input). Release
    // builds stay lean. Force it anywhere with SLINT_EMIT_DEBUG_INFO=1.
    let debug = std::env::var("PROFILE").as_deref() != Ok("release")
        || std::env::var_os("SLINT_EMIT_DEBUG_INFO").is_some();
    if debug {
        config = config.with_debug_info(true);
    }
    slint_build::compile_with_config("ui/app.slint", config).unwrap();
}
