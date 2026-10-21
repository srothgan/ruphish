//! Compile the frontend catalogs with checked translation keys.
use leptos_i18n_build::{Config, ParseOptions, TranslationsInfos};
use std::{error::Error, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo::rerun-if-changed=build.rs");
    let config = Config::new("en")?
        .add_locale("de")?
        .parse_options(ParseOptions::default().interpolate_display(true));
    let translations = TranslationsInfos::parse(config)?;
    translations.emit_diagnostics();
    translations.rerun_if_locales_changed();
    translations.generate_i18n_module(PathBuf::from(std::env::var("OUT_DIR")?).join("i18n"))?;
    Ok(())
}
