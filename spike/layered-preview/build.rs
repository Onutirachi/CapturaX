fn main() {
    // Translations are bundled and picked at runtime (T036); `.po` entries carry no context.
    let config = slint_build::CompilerConfiguration::new()
        .with_bundled_translations("lang")
        .with_default_translation_context(slint_build::DefaultTranslationContext::None);
    slint_build::compile_with_config("ui/app.slint", config).expect("Slint UI must compile");
}
