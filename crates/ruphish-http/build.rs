//! Embed the shared starter manifest without requiring Node or runtime file reads.
use serde::Deserialize;
use std::{collections::HashSet, error::Error, fmt::Write, path::PathBuf};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    id: String,
    name: String,
    brand: String,
    kind: String,
    html: String,
    script: Option<String>,
    subject: Option<String>,
    text: Option<String>,
    company_logo: Option<Logo>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Logo {
    selector: String,
    height: u16,
    context: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=starter-templates/catalog.json");
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?).join("starter-templates");
    let entries: Vec<Entry> = serde_json::from_slice(&std::fs::read(root.join("catalog.json"))?)?;
    let include = |path: &str| format!("include_str!({:?})", root.join(path));
    let mut ids = HashSet::new();
    let mut output = String::from("const CATALOG: &[Source] = &[\n");
    for entry in entries {
        if !ids.insert(entry.id.clone()) {
            return Err(format!("Duplicate starter ID: {}", entry.id).into());
        }
        let kind = match entry.kind.as_str() {
            "email" => "Email",
            "landing_page" => "LandingPage",
            "learning_page" => "LearningPage",
            other => return Err(format!("Unknown starter kind: {other}").into()),
        };
        let script = entry.script.as_deref().map_or_else(
            || "None".to_owned(),
            |path| format!("Some({})", include(path)),
        );
        let subject = entry
            .subject
            .as_deref()
            .map_or_else(|| "\"\"".to_owned(), include);
        let text = entry
            .text
            .as_deref()
            .map_or_else(|| "\"\"".to_owned(), include);
        let logo = entry.company_logo.map_or_else(
            || "None".to_owned(),
            |logo| {
                format!(
                    "Some(({:?}, {}, {:?}))",
                    logo.selector, logo.height, logo.context
                )
            },
        );
        writeln!(
            output,
            "Source {{ id: {:?}, name: {:?}, brand: {:?}, kind: Kind::{kind}, html: {}, script: {script}, subject: {subject}, text: {text}, logo: {logo} }},",
            entry.id,
            entry.name,
            entry.brand,
            include(&entry.html)
        )?;
    }
    output.push_str("];\n");
    std::fs::write(
        PathBuf::from(std::env::var("OUT_DIR")?).join("starter_catalog.rs"),
        output,
    )?;
    Ok(())
}
