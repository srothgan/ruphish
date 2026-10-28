//! Bounded, in-memory page bundles. Embedded assets inherit the page's existing lifecycle.
use crate::resources::MAX_DOCUMENT;
use base64::{Engine, engine::general_purpose::STANDARD};
use cssparser::{Parser, ToCss, Token};
use ruphish_application::resources::Error;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Read},
};

fn invalid(message: &str) -> Error {
    Error::Invalid(message.into())
}

/// Import index.html and its declared local resources without extracting files or fetching URLs.
pub(super) fn import(encoded: &str) -> Result<String, Error> {
    if encoded.len() > MAX_DOCUMENT * 4 / 3 + 4 {
        return Err(invalid("Page bundle exceeds 8 MiB"));
    }
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|_| invalid("Invalid page bundle encoding"))?;
    if bytes.len() > MAX_DOCUMENT {
        return Err(invalid("Page bundle exceeds 8 MiB"));
    }
    let mut zip =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| invalid("Invalid ZIP page bundle"))?;
    if zip.len() > 256 {
        return Err(invalid("Page bundle exceeds 256 entries"));
    }
    let root = url::Url::parse("https://bundle.invalid/").map_err(|_| Error::Internal)?;
    let mut files = BTreeMap::new();
    let mut total = 0usize;
    for index in 0..zip.len() {
        let entry = zip
            .by_index(index)
            .map_err(|_| invalid("Unreadable page bundle entry"))?;
        let name = entry
            .name()
            .map_err(|_| invalid("Invalid page bundle filename"))?
            .into_owned();
        if name.starts_with('/')
            || name.contains(['\\', ':', '?', '#', '%'])
            || name.split('/').any(|part| matches!(part, "." | ".."))
            || entry
                .unix_mode()
                .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err(invalid("Unsafe page bundle path"));
        }
        if entry.is_dir() {
            continue;
        }
        if entry.size() > MAX_DOCUMENT as u64 {
            return Err(invalid("Expanded page bundle exceeds 8 MiB"));
        }
        let mut content = Vec::new();
        entry
            .take((MAX_DOCUMENT - total + 1) as u64)
            .read_to_end(&mut content)
            .map_err(|_| invalid("Unreadable page bundle content"))?;
        total += content.len();
        if total > MAX_DOCUMENT {
            return Err(invalid("Expanded page bundle exceeds 8 MiB"));
        }
        let path = root
            .join(&name)
            .map_err(|_| invalid("Invalid page bundle path"))?
            .path()
            .to_owned();
        if files.insert(path, content).is_some() {
            return Err(invalid("Duplicate page bundle path"));
        }
    }
    let source = files
        .get("/index.html")
        .ok_or_else(|| invalid("Page bundle requires index.html at its root"))?;
    let source =
        std::str::from_utf8(source).map_err(|_| invalid("Page bundle HTML must be UTF-8"))?;
    let document = scraper::Html::parse_document(source);
    let mut bundle = Bundle {
        files: &files,
        active: BTreeSet::new(),
    };
    let mut base = root.join("index.html").map_err(|_| Error::Internal)?;
    let selector = scraper::Selector::parse("base[href]").map_err(|_| Error::Internal)?;
    if let Some(value) = document
        .select(&selector)
        .next()
        .and_then(|el| el.value().attr("href"))
    {
        base = base
            .join(value)
            .map_err(|_| invalid("Invalid bundle base URL"))?;
    }
    // Mutable resource resolution is shared by all handlers and recursively processes CSS.
    let bundle = std::cell::RefCell::new(&mut bundle);
    let settings = lol_html::Settings::new()
        .with_memory_settings(
            lol_html::MemorySettings::new().with_max_allowed_memory_usage(MAX_DOCUMENT),
        )
        .append_element_content_handler(lol_html::element!("base", |el| {
            el.remove();
            Ok(())
        }))
        .append_element_content_handler(lol_html::element!(
            "img[src], script[src], link[href], source[src], video[poster]",
            |el| {
                let attribute = if el.tag_name() == "link" {
                    "href"
                } else if el.tag_name() == "video" {
                    "poster"
                } else {
                    "src"
                };
                let value = el.get_attribute(attribute).unwrap_or_default();
                if el.tag_name() == "script"
                    && el
                        .get_attribute("type")
                        .is_some_and(|kind| kind.eq_ignore_ascii_case("module"))
                    && base
                        .join(&value)
                        .is_ok_and(|url| url.host_str() == Some("bundle.invalid"))
                {
                    return Err(
                        "Bundle local JavaScript modules into a classic script before import"
                            .into(),
                    );
                }
                let url = bundle.borrow_mut().resource(&base, &value, 0)?;
                if url != value && url.starts_with("data:") {
                    // Recursive CSS embedding changes bytes. External integrity metadata is
                    // retained; packaged resources are now part of the owned page document.
                    el.remove_attribute("integrity");
                }
                el.set_attribute(attribute, &url)?;
                Ok(())
            }
        ))
        .append_element_content_handler(lol_html::element!("[style]", |el| {
            let value = el.get_attribute("style").unwrap_or_default();
            el.set_attribute("style", &bundle.borrow_mut().css(&base, &value, 0)?)?;
            Ok(())
        }))
        .append_element_content_handler(lol_html::element!("img[srcset], source[srcset]", |el| {
            let source = el.get_attribute("srcset").unwrap_or_default();
            let mut rest = source.as_str();
            let mut candidates = Vec::new();
            while !rest
                .trim_start_matches(|c: char| c.is_ascii_whitespace() || c == ',')
                .is_empty()
            {
                rest = rest.trim_start_matches(|c: char| c.is_ascii_whitespace() || c == ',');
                let end = rest
                    .find(|c: char| c.is_ascii_whitespace())
                    .unwrap_or(rest.len());
                let raw = &rest[..end];
                rest = &rest[end..];
                let url = bundle
                    .borrow_mut()
                    .resource(&base, raw.trim_end_matches(','), 0)?;
                let descriptor = if raw.ends_with(',') {
                    ""
                } else {
                    let end = rest.find(',').unwrap_or(rest.len());
                    let descriptor = rest[..end].trim();
                    rest = &rest[end..];
                    descriptor
                };
                candidates.push(format!("{url} {descriptor}"));
            }
            el.set_attribute("srcset", &candidates.join(", "))?;
            Ok(())
        }));
    // A style text handler can receive chunks; transform each complete style element
    // before HTML rewriting so CSS tokens are never split at streaming boundaries.
    let styles = scraper::Selector::parse("style").map_err(|_| Error::Internal)?;
    let mut css = document
        .select(&styles)
        .map(|el| bundle.borrow_mut().css(&base, &el.inner_html(), 0))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter();
    let settings =
        settings.append_element_content_handler(lol_html::element!("style", move |el| {
            let text = css
                .next()
                .ok_or_else(|| invalid("Invalid bundle style element"))?;
            // CSS strings/comments must not be able to terminate their HTML raw-text container.
            el.set_inner_content(
                &style_text(&text),
                lol_html::html_content::ContentType::Html,
            );
            Ok(())
        }));
    let source = document.html();
    let mut output = Vec::new();
    let mut oversized = false;
    let mut rewriter = lol_html::HtmlRewriter::new(settings, |chunk: &[u8]| {
        if output.len().saturating_add(chunk.len()) > MAX_DOCUMENT {
            oversized = true;
        } else {
            output.extend_from_slice(chunk);
        }
    });
    rewriter
        .write(source.as_bytes())
        .map_err(|_| invalid("Page bundle contains missing, unsupported or invalid resources"))?;
    rewriter
        .end()
        .map_err(|_| invalid("Page bundle contains missing, unsupported or invalid resources"))?;
    if oversized {
        return Err(invalid("Embedded page exceeds 8 MiB"));
    }
    String::from_utf8(output).map_err(|_| Error::Internal)
}

struct Bundle<'a> {
    files: &'a BTreeMap<String, Vec<u8>>,
    active: BTreeSet<String>,
}

fn style_text(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    for (index, character) in source.char_indices() {
        if character == '<'
            && source[index..]
                .get(..7)
                .is_some_and(|text| text.eq_ignore_ascii_case("</style"))
        {
            output.push_str("\\3c ");
        } else {
            output.push(character);
        }
    }
    output
}
impl Bundle<'_> {
    fn resource(&mut self, base: &url::Url, value: &str, depth: usize) -> Result<String, Error> {
        if value.starts_with('#') || value.starts_with("data:") {
            return Ok(value.into());
        }
        let url = base
            .join(value)
            .map_err(|_| invalid("Invalid page asset URL"))?;
        if !matches!(url.scheme(), "https" | "http") {
            return Err(invalid("Unsupported page asset URL"));
        }
        if url.host_str() != Some("bundle.invalid") {
            return Ok(url.to_string());
        }
        if depth >= 24 || !self.active.insert(url.path().into()) {
            return Err(invalid("Cyclic or excessively nested CSS dependency"));
        }
        let content = self
            .files
            .get(url.path())
            .ok_or_else(|| invalid("A referenced page asset is missing from the bundle"))?;
        let mime = ruphish_domain::landing::asset_mime(url.path())
            .ok_or_else(|| invalid("Unsupported page asset type"))?;
        let css;
        let content = if mime == "text/css" {
            css = self.css(
                &url,
                std::str::from_utf8(content).map_err(|_| invalid("Stylesheets must be UTF-8"))?,
                depth + 1,
            )?;
            css.as_bytes()
        } else {
            content.as_slice()
        };
        let mut result = format!("data:{mime};base64,{}", STANDARD.encode(content));
        if let Some(fragment) = url.fragment() {
            result.push('#');
            result.push_str(fragment);
        }
        self.active.remove(url.path());
        if result.len() > MAX_DOCUMENT {
            return Err(invalid("Embedded asset exceeds 8 MiB"));
        }
        Ok(result)
    }
    fn css(&mut self, base: &url::Url, source: &str, depth: usize) -> Result<String, Error> {
        self.css_tokens(base, &mut Parser::new(source), depth, false)
    }
    fn css_tokens(
        &mut self,
        base: &url::Url,
        parser: &mut Parser<'_>,
        depth: usize,
        image_set: bool,
    ) -> Result<String, Error> {
        if depth >= 64 {
            return Err(invalid("CSS nesting exceeds its limit"));
        }
        let mut output = String::new();
        let mut import = false;
        while let Ok(token) = parser.next_including_whitespace_and_comments().cloned() {
            match &token {
                Token::AtKeyword(name) if name.eq_ignore_ascii_case("import") => {
                    output.push_str("@import");
                    import = true;
                    continue;
                }
                Token::UnquotedUrl(value) => {
                    output.push_str("url(");
                    cssparser::serialize_string(&self.resource(base, value, depth)?, &mut output)
                        .map_err(|_| Error::Internal)?;
                    output.push(')');
                }
                Token::QuotedString(value) if import || image_set => {
                    cssparser::serialize_string(&self.resource(base, value, depth)?, &mut output)
                        .map_err(|_| Error::Internal)?;
                }
                Token::Function(name) if name.eq_ignore_ascii_case("url") => {
                    let value = parser
                        .parse_nested_block(|nested| {
                            nested
                                .expect_string_cloned()
                                .map_err(cssparser::ParseError::<()>::from)
                        })
                        .map_err(|_| invalid("Invalid CSS URL"))?;
                    output.push_str("url(");
                    cssparser::serialize_string(&self.resource(base, &value, depth)?, &mut output)
                        .map_err(|_| Error::Internal)?;
                    output.push(')');
                }
                Token::Function(_)
                | Token::ParenthesisBlock
                | Token::SquareBracketBlock
                | Token::CurlyBracketBlock => {
                    token.to_css(&mut output).map_err(|_| Error::Internal)?;
                    let image_set = matches!(&token, Token::Function(name) if name.eq_ignore_ascii_case("image-set") || name.eq_ignore_ascii_case("-webkit-image-set"));
                    let inner = parser
                        .parse_nested_block(|nested| {
                            self.css_tokens(base, nested, depth + 1, image_set)
                                .map_err(cssparser::ParseError::<Error>::custom)
                        })
                        .map_err(|_| invalid("Invalid CSS dependency"))?;
                    output.push_str(&inner);
                    output.push(match token {
                        Token::SquareBracketBlock => ']',
                        Token::CurlyBracketBlock => '}',
                        _ => ')',
                    });
                }
                Token::BadUrl(_) | Token::BadString(_) => {
                    return Err(invalid("Invalid CSS syntax"));
                }
                _ => token.to_css(&mut output).map_err(|_| Error::Internal)?,
            }
            if !matches!(token, Token::WhiteSpace(_) | Token::Comment(_)) {
                import = false;
            }
            if output.len() > MAX_DOCUMENT {
                return Err(invalid("Embedded stylesheet exceeds 8 MiB"));
            }
        }
        Ok(output)
    }
}
