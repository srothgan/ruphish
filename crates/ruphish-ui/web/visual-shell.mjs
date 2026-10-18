// Trusted application chrome only. Authored HTML is passed separately as data.
export function visualFrameDocument(script, css) {
  const escape = text => text.replaceAll('&', '&amp;').replaceAll('"', '&quot;').replaceAll('<', '&lt;');
  return `<!doctype html><html lang="en"><head><meta charset="utf-8">
    <meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src ${escape(script)}; style-src ${escape(css)} 'unsafe-inline'; img-src data: blob:; frame-src about:; form-action 'none'; base-uri 'none'">
    <link rel="stylesheet" href="${escape(css)}">
    <style>
      *{box-sizing:border-box}body{font:15px/1.5 'Segoe UI',sans-serif;margin:0;padding:16px;color:#18283c;background:#fff}
      p{margin:0 0 12px}button,select,input{font:inherit;padding:7px 10px;color:inherit;border:1px solid #8191a6;border-radius:4px;background:#fff}
      button{cursor:pointer}button:disabled,select:disabled{cursor:default;opacity:.55}button.primary{color:#fff;background:#2459ad;border-color:#2459ad}
      label{display:block;font-weight:600}label input,label select{display:block;width:100%;margin-top:4px}
      [hidden]{display:none!important}fieldset{padding:0;margin:0;border:0;min-width:0}#status[data-error]{color:var(--color-danger);background:#fff;padding:12px;border-left:3px solid var(--color-danger)}
      #variable{min-width:180px}#variable-label{margin:12px 0;max-width:360px}.actions{display:flex;gap:8px;margin-top:16px}
      details{margin-top:20px;border-top:1px solid #d9e0ea;padding-top:12px}summary{cursor:pointer;font-weight:600}.form-grid{display:grid;grid-template-columns:1fr 1fr;gap:12px;margin:12px 0}
      :focus-visible{outline:3px solid #2459ad;outline-offset:2px}
    </style></head><body>
    <p id="status" role="status">Loading the visual editor…</p>
    <fieldset id="editing" disabled hidden>
      <p>Select text in the document, then use <strong>B</strong> for bold, <em>I</em> for italic, or another toolbar control. Formatting appears immediately.</p>
      <textarea id="visual" aria-label="Editable message content"></textarea>
      <label id="variable-label">Insert a recipient field<select id="variable" disabled><option value="">Choose a field…</option></select></label>
      <details><summary>Form controls (for landing pages)</summary>
        <p>Add a form, or click an existing input in the document and choose “Load selected field”.</p>
        <button id="load-field" type="button">Load selected field</button>
        <form id="form"><div class="form-grid">
          <label>Submission URL<input name="action" value="{{.URL}}" required></label>
          <label>Method<select name="method"><option value="post">POST</option><option value="get">GET</option></select></label>
          <label>Field name<input name="name" value="email" required></label>
          <label>Visible label<input name="label" value="Email" required></label>
          <label>Field type<select name="type"><option>email</option><option>text</option><option>password</option><option>hidden</option></select></label>
        </div><button type="submit">Insert form</button> <button id="update-field" type="button">Update selected field</button></form>
      </details>
      <div class="actions"><button id="apply" class="primary" disabled>Apply visual changes</button></div>
      <p>Apply returns to HTML source and keeps your edits. Cancel visual edits above returns without changing the source.</p>
    </fieldset>
    <script src="${escape(script)}"></script></body></html>`;
}
