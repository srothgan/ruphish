//! Exercise the production WASM download boundary and actual browser Blob bytes.
#![cfg(target_arch = "wasm32")]
#![allow(clippy::unwrap_used)]

use wasm_bindgen::prelude::*;
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen(inline_js = r#"
let originalDocument, originalTimer, anchor, cleanup, revoked;
export function installDownload() {
  originalDocument = globalThis.document;
  originalTimer = globalThis.setTimeout;
  revoked = false;
  anchor = {click() { this.clicked = true; }, remove() { this.removed = true; }};
  globalThis.document = {createElement(tag) {
    if (tag !== 'a') throw new Error('unexpected element');
    return anchor;
  }, body: {append(value) { if (value !== anchor) throw new Error('wrong anchor'); }}};
  // Keep real Blob/object URL creation; capture the actual object via fetch below.
  globalThis.setTimeout = callback => { cleanup = callback; return 1; };
}
export async function downloadedText() {
  if (!anchor.clicked || !anchor.removed || anchor.download !== 'recipient-template.csv')
    throw new Error('download was not initiated');
  const response = await fetch(anchor.href);
  if (response.headers.get('content-type') !== 'text/csv;charset=utf-8')
    throw new Error('wrong content type');
  const text = await response.text();
  cleanup();
  try { await fetch(anchor.href); } catch { revoked = true; }
  if (!revoked) throw new Error('object URL was not released');
  return text;
}
export function breakDownload() {
  anchor.click = () => { throw new Error('private platform diagnostic'); };
}
export async function failedDownloadReleased() {
  try { await fetch(anchor.href); } catch { return true; }
  return false;
}
export function restoreDownload() {
  globalThis.document = originalDocument;
  globalThis.setTimeout = originalTimer;
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = installDownload)]
    fn install_download();
    #[wasm_bindgen(js_name = downloadedText)]
    async fn downloaded_text() -> JsValue;
    #[wasm_bindgen(js_name = breakDownload)]
    fn break_download();
    #[wasm_bindgen(js_name = failedDownloadReleased)]
    async fn failed_download_released() -> JsValue;
    #[wasm_bindgen(js_name = restoreDownload)]
    fn restore_download();
}

#[wasm_bindgen_test]
async fn recipient_template_downloads_exact_importable_bytes_and_reports_browser_failure() {
    install_download();
    ruphish_ui::browser_http::download_recipient_template().unwrap();
    let text = downloaded_text().await.as_string().unwrap();
    assert_eq!(text, include_str!("../assets/recipient-template.csv"));
    assert_eq!(
        text.lines().collect::<Vec<_>>(),
        [
            "First Name,Last Name,Email,Position",
            "Alex,Rivera,alex@example.test,Operations",
            "Sam,Lee,sam@example.test,Support",
        ]
    );
    break_download();
    let error = ruphish_ui::browser_http::download_recipient_template().unwrap_err();
    assert!(!error.is_empty());
    assert!(!error.contains("private platform diagnostic"));
    assert_eq!(failed_download_released().await.as_bool(), Some(true));
    restore_download();
}
