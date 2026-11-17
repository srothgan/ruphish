//! Archive decoding and generated archives that reach HTML/CSS processing.
use base64::{Engine, engine::general_purpose::STANDARD};
use ruphish_adapters::{imports::Tools, outbound::Policy, resources::MAX_DOCUMENT};
use ruphish_application::resources::{ContentTools, Error};
use std::io::{Cursor, Write};

fn import(bytes: &[u8]) -> Result<String, Error> {
    Tools {
        policy: Policy::default(),
    }
    .page_bundle(&STANDARD.encode(bytes))
}

fn archive(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, content) in files {
        writer
            .start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(content).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn bounded(result: Result<String, Error>) {
    match result {
        Ok(html) => assert!(html.len() <= MAX_DOCUMENT),
        Err(Error::Invalid(_)) => {}
        Err(error) => panic!("Unexpected bundle error: {error}"),
    }
}

/// Mutate ZIP bytes, including central directory and compression metadata.
pub fn raw(data: &[u8]) {
    if data.len() <= 1024 * 1024 {
        bounded(import(data));
    }
}

/// Generate valid containers so mutations reach paths, HTML, CSS and embedding.
pub fn structured(data: &[u8]) {
    let Some((&mode, payload)) = data.split_first() else {
        return;
    };
    if payload.len() > 32 * 1024 {
        return;
    }
    let text = String::from_utf8_lossy(payload);
    match mode % 8 {
        0 => bounded(import(&archive(&[
            ("index.html", payload),
            ("style.css", b"p { color: red }"),
        ]))),
        1 => {
            // The opaque image payload must survive embedding byte for byte.
            let html = import(&archive(&[
                ("index.html", b"<img id='asset' src='image.png'>"),
                ("image.png", payload),
            ]))
            .unwrap();
            let document = scraper::Html::parse_document(&html);
            let selector = scraper::Selector::parse("img#asset").unwrap();
            let src = document
                .select(&selector)
                .next()
                .unwrap()
                .value()
                .attr("src")
                .unwrap();
            assert_eq!(
                STANDARD
                    .decode(src.strip_prefix("data:image/png;base64,").unwrap())
                    .unwrap(),
                payload
            );
            assert!(html.len() <= MAX_DOCUMENT);
        }
        2 => {
            // Arbitrary CSS inside a real stylesheet with an available local asset.
            bounded(import(&archive(&[
                (
                    "index.html",
                    b"<link rel='stylesheet' href='style.css'><style>p { color: red }</style>",
                ),
                ("style.css", text.as_bytes()),
                ("image.png", b"synthetic"),
            ])));
        }
        3 => {
            let filename = format!(
                "../{}.png",
                payload
                    .iter()
                    .take(32)
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>()
            );
            assert!(matches!(
                import(&archive(&[
                    ("index.html", b"<p>safe</p>"),
                    (&filename, payload)
                ])),
                Err(Error::Invalid(_))
            ));
        }
        4 => {
            // A cycle is invalid regardless of its bounded length.
            let count = 1 + payload.first().copied().unwrap_or(0) as usize % 12;
            let names: Vec<_> = (0..count).map(|i| format!("{i}.css")).collect();
            let contents: Vec<_> = (0..count)
                .map(|i| format!("@import '{}.css';", (i + 1) % count))
                .collect();
            let mut files = vec![(
                "index.html",
                b"<link rel='stylesheet' href='0.css'>".as_slice(),
            )];
            files.extend(
                names
                    .iter()
                    .zip(&contents)
                    .map(|(name, content)| (name.as_str(), content.as_bytes())),
            );
            assert!(matches!(import(&archive(&files)), Err(Error::Invalid(_))));
        }
        5 => assert!(matches!(
            import(&archive(&[("other.html", payload)])),
            Err(Error::Invalid(_))
        )),
        6 => {
            // Exercise CSS token rewriting and srcset with an available image.
            let css = format!("p {{ background: url('image.png'); }} /* {} */", text);
            bounded(import(&archive(&[
                ("index.html", b"<link rel='stylesheet' href='style.css'><img srcset='image.png 1x, image.png 2x'>"),
                ("style.css", css.as_bytes()),
                ("image.png", payload),
            ])));
        }
        _ => {
            // CSS escape decoding must not create HTML siblings outside a style element.
            let escape = if payload.first().copied().unwrap_or(0) & 1 == 0 {
                "\\3c "
            } else {
                "\\00003c"
            };
            let source = format!(
                r#"<style>p::after {{content:"{escape}/style>{escape}p id=fuzz-sentinel>injected{escape}/p>"}}</style><p id=kept>visible</p>"#
            );
            let output = import(&archive(&[("index.html", source.as_bytes())])).unwrap();
            let document = scraper::Html::parse_document(&output);
            assert_eq!(
                document
                    .select(&scraper::Selector::parse("#fuzz-sentinel").unwrap())
                    .count(),
                0
            );
            assert_eq!(
                document
                    .select(&scraper::Selector::parse("p#kept").unwrap())
                    .count(),
                1
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structured_cases_reach_embedding_and_rejections() {
        for mode in 0..8 {
            let mut input = vec![mode];
            input.extend_from_slice(b"<p>synthetic</p>");
            structured(&input);
        }
        for input in [
            b"".as_slice(),
            b"PK\x03\x04broken",
            include_bytes!("../seeds/page_bundle_raw/minimal.zip"),
        ] {
            raw(input);
        }
        assert!(import(include_bytes!("../seeds/page_bundle_raw/minimal.zip")).is_ok());
    }

    #[test]
    fn expanded_size_and_entry_limits_reject_valid_containers() {
        let huge = vec![b'x'; MAX_DOCUMENT + 1];
        let compressed = archive(&[("index.html", &huge)]);
        assert!(compressed.len() < MAX_DOCUMENT);
        assert!(
            matches!(import(&compressed), Err(Error::Invalid(message)) if message == "Expanded page bundle exceeds 8 MiB")
        );
        let names: Vec<_> = (0..256).map(|i| format!("{i}.txt")).collect();
        let mut files = vec![("index.html", b"<p>valid</p>".as_slice())];
        files.extend(
            names
                .iter()
                .take(255)
                .map(|name| (name.as_str(), b"".as_slice())),
        );
        assert!(import(&archive(&files)).is_ok(), "256 valid entries fit");
        files.push((&names[255], b""));
        assert!(matches!(import(&archive(&files)), Err(Error::Invalid(_))));
    }
}
