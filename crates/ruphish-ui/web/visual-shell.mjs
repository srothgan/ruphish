import { editorText } from './editor-i18n.mjs';
// Trusted application chrome only. Authored HTML is passed separately as data.
export function visualFrameDocument(script, css, locale = 'en', allowForms = false) {
  const escape = (text) =>
    text.replaceAll('&', '&amp;').replaceAll('"', '&quot;').replaceAll('<', '&lt;');
  const text = (key) => escape(editorText(locale, key));
  const label = (key) => `<span data-editor-text="${key}">${text(key)}</span>`;
  return `<!doctype html><html lang="${escape(locale)}"><head><meta charset="utf-8">
    <meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src ${escape(script)}; style-src ${escape(css)} 'unsafe-inline'; img-src data: blob:; frame-src about:; form-action 'none'; base-uri 'none'">
    <link rel="stylesheet" href="${escape(css)}">
    <style>
      *{box-sizing:border-box}body{font:15px/1.5 'Segoe UI',sans-serif;margin:0;padding:16px;color:#18283c;background:#fff}
      p{margin:0 0 12px}button,select,input{font:inherit;padding:7px 10px;color:inherit;border:1px solid #8191a6;border-radius:4px;background:#fff}
      button{cursor:pointer}button:disabled,select:disabled{cursor:default;opacity:.55}button.primary{color:#fff;background:#2459ad;border-color:#2459ad}
      label{display:block;font-weight:600}label input,label select{display:block;width:100%;margin-top:4px}
      [hidden]{display:none!important}fieldset{padding:0;margin:0;border:0;min-width:0}#status[data-error]{color:var(--danger);background:#fff;padding:12px;border:1px solid var(--danger)}
      details{margin-top:20px;border-top:1px solid #d9e0ea;padding-top:12px}summary{cursor:pointer;font-weight:600}.form-grid{display:grid;grid-template-columns:1fr 1fr;gap:12px;margin:12px 0}
      :focus-visible{outline:3px solid #2459ad;outline-offset:2px}
    </style></head><body>
    <p id="status" role="status">${label('frame_loading')}</p>
    <fieldset id="editing" disabled hidden>
      <textarea id="visual" data-editor-label="editable_content" aria-label="${text('editable_content')}"></textarea>
      ${
        allowForms
          ? `<details><summary data-editor-text="form_controls">${text('form_controls')}</summary>
        <p>${label('form_help')}</p>
        <button id="load-field" type="button" data-editor-text="load_field">${text('load_field')}</button>
        <form id="form"><div class="form-grid">
          <label>${label('submission_url')}<input name="action" value="{{.URL}}" required></label>
          <label>${label('method')}<select name="method"><option value="post">POST</option><option value="get">GET</option></select></label>
          <label>${label('field_name')}<input name="name" value="email" required></label>
          <label>${label('visible_label')}<input name="label" value="${text('field_email')}" required></label>
          <label>${label('field_type')}<select name="type"><option value="email" data-editor-text="field_email">${text('field_email')}</option><option value="text" data-editor-text="field_text">${text('field_text')}</option><option value="password" data-editor-text="field_password">${text('field_password')}</option><option value="hidden" data-editor-text="field_hidden">${text('field_hidden')}</option></select></label>
        </div><button type="submit" data-editor-text="insert_form">${text('insert_form')}</button> <button id="update-field" type="button" data-editor-text="update_field">${text('update_field')}</button></form>
      </details>`
          : ''
      }
    </fieldset>
    <script src="${escape(script)}"></script></body></html>`;
}
