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

#[path = "asset_distribution.rs"]
#[cfg(feature = "embedded-assets")]
mod asset_distribution;

fn main() -> Result<(), Box<dyn Error>> {
    #[cfg(feature = "embedded-assets")]
    embed_admin()?;
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

#[cfg(feature = "embedded-assets")]
fn embed_admin() -> Result<(), Box<dyn Error>> {
    println!("cargo::rerun-if-changed=asset_distribution.rs");
    use std::io::Write as _;
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?).join("../../dist/admin");
    println!("cargo::rerun-if-changed={}", root.display());
    let assets = asset_distribution::load(&root).map_err(|error| {
        format!("Cannot embed admin assets; run trunk build --release --locked first: {error}")
    })?;
    let output_dir = PathBuf::from(std::env::var("OUT_DIR")?).join("admin");
    std::fs::create_dir_all(&output_dir)?;
    let mut source = String::from("vec![\n");
    for (index, (name, bytes)) in assets.into_iter().enumerate() {
        // Copy the validated snapshot into OUT_DIR: include_bytes must consume
        // those same bytes even if another Trunk build later replaces dist/admin.
        let identity = output_dir.join(format!("{index}.identity"));
        std::fs::write(&identity, &bytes)?;
        let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
        gzip.write_all(&bytes)?;
        let gzip = gzip.finish()?;
        let mut brotli = Vec::new();
        let parameters = brotli::enc::BrotliEncoderParams {
            quality: 9,
            ..Default::default()
        };
        brotli::BrotliCompress(&mut bytes.as_slice(), &mut brotli, &parameters)?;
        let gzip_path = output_dir.join(format!("{index}.gz"));
        let brotli_path = output_dir.join(format!("{index}.br"));
        std::fs::write(&gzip_path, &gzip)?;
        std::fs::write(&brotli_path, &brotli)?;
        let representation = |path: &std::path::Path, bytes: &[u8]| {
            use sha2::{Digest, Sha256};
            format!(
                "crate::static_asset::Representation {{ bytes: axum::body::Bytes::from_static(include_bytes!({path:?})), etag: {:?}.into() }}",
                format!("\"{}\"", hex::encode(Sha256::digest(bytes)))
            )
        };
        writeln!(
            source,
            "({name:?}.into(), Asset {{ identity: {}, gzip: Some({}), brotli: Some({}) }}),",
            representation(&identity, &bytes),
            representation(&gzip_path, &gzip),
            representation(&brotli_path, &brotli)
        )?;
    }
    source.push(']');
    std::fs::write(
        output_dir
            .parent()
            .ok_or("Missing build output parent")?
            .join("admin_assets.rs"),
        source,
    )?;
    Ok(())
}
