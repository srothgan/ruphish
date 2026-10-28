import { EditorError } from './editor-i18n.mjs';
import { TemplateVault } from './template-html.mjs';

// One local transaction: no normalized document is saved simply by opening a mode.
export class VisualSession {
  constructor(editor, source) {
    this.editor = editor;
    this.original = source;
    this.vault = new TemplateVault(source);
    const masked = this.vault.mask(source);
    const parsed = new editor.ow.DOMParser().parseFromString(masked, 'text/html');
    this.doctype = parsed.doctype
      ? new editor.ow.XMLSerializer().serializeToString(parsed.doctype)
      : '';
    this.bodyEditable = parsed.body.getAttribute('contenteditable');
    this.bodyClasses = new Set(parsed.body.classList);
    this.htmlClasses = new Set(parsed.documentElement.classList);
    this.policyMarker = editor.o.iframeDoctype.match(/data-ruphish-editor-policy="([^"]+)"/)?.[1];
    editor.value = masked;
    const body = editor.editor.ownerDocument.body;
    this.vault.decorate(editor.editor.ownerDocument);
    this.initial = this.serialize();
    body.addEventListener('click', (event) => {
      const field = event.target.closest?.('input');
      if (field) {
        this.selectField(field);
      }
      if (event.target.closest?.('a,button,input[type=submit]')) {
        event.preventDefault();
      }
    });
    body.addEventListener('submit', (event) => event.preventDefault());
  }

  serialize() {
    const document = this.editor.editor.ownerDocument;
    const copy = document.cloneNode(true);
    // The runtime preview policy is never part of the authored training document.
    if (this.policyMarker) {
      copy.querySelector(`meta[data-ruphish-editor-policy="${this.policyMarker}"]`)?.remove();
    }
    if (this.bodyEditable === null) {
      copy.body.removeAttribute('contenteditable');
    } else {
      copy.body.setAttribute('contenteditable', this.bodyEditable);
    }
    if (!this.bodyClasses.has('jodit-wysiwyg')) {
      copy.body.classList.remove('jodit-wysiwyg');
    }
    if (!this.htmlClasses.has('jodit')) {
      copy.documentElement.classList.remove('jodit');
    }
    if (!copy.body.className) {
      copy.body.removeAttribute('class');
    }
    if (!copy.documentElement.className) {
      copy.documentElement.removeAttribute('class');
    }
    return this.doctype + this.vault.export(copy);
  }

  result() {
    const value = this.serialize();
    return value === this.initial ? this.original : value;
  }

  insertVariable(name) {
    const marker = this.vault.mask(`{{.${name}}}`);
    this.editor.s.insertHTML(marker);
    this.vault.decorate(this.editor.editor.ownerDocument);
    const chip = this.editor.editor.querySelector(`[data-ruphish-field="${marker}"]`);
    if (chip) {
      this.editor.s.setCursorAfter(chip);
    }
  }

  insertForm({ action, method, name, label, type, submitLabel }) {
    const document = this.editor.editor.ownerDocument;
    const form = document.createElement('form');
    form.setAttribute('action', this.vault.mask(action));
    form.setAttribute('method', method);
    const fieldLabel = document.createElement('label');
    fieldLabel.textContent = label;
    const field = document.createElement('input');
    field.setAttribute('name', name);
    field.setAttribute('type', type);
    fieldLabel.append(field);
    const submit = document.createElement('button');
    submit.setAttribute('type', 'submit');
    submit.textContent = submitLabel;
    form.append(fieldLabel, submit);
    this.editor.s.insertHTML(form.outerHTML);
  }

  selectField(field) {
    if (field.ownerDocument !== this.editor.ed || !field.closest('form')) {
      throw new EditorError('select_form_field');
    }
    this.selectedField = field;
  }

  selectedProperties() {
    const field = this.selectedField;
    if (!field?.isConnected) {
      throw new EditorError('select_existing_field');
    }
    return {
      action: this.vault.restore(field.closest('form').getAttribute('action') ?? ''),
      method: field.closest('form').getAttribute('method') ?? 'get',
      name: field.getAttribute('name') ?? '',
      type: field.getAttribute('type') ?? 'text',
      label: [...(field.labels?.[0]?.childNodes ?? [])]
        .filter((node) => node.nodeType === 3)
        .map((node) => node.data)
        .join(''),
    };
  }

  updateSelected({ action, method, name, label, type }) {
    this.selectedProperties();
    const field = this.selectedField;
    const form = field.closest('form');
    form.setAttribute('action', this.vault.mask(action));
    form.setAttribute('method', method);
    field.setAttribute('name', name);
    field.setAttribute('type', type);
    let fieldLabel = field.labels?.[0];
    if (!fieldLabel) {
      fieldLabel = this.editor.ed.createElement('label');
      field.replaceWith(fieldLabel);
      fieldLabel.append(field);
    }
    for (const node of [...fieldLabel.childNodes]) {
      if (node.nodeType === 3) {
        node.remove();
      }
    }
    fieldLabel.prepend(this.editor.ed.createTextNode(label));
  }
}

export function visualOptions(window, source = '', locale = 'en') {
  let policyMarker = 'ruphish-editor-policy';
  while (source.includes(policyMarker)) {
    policyMarker += 'X';
  }
  return {
    language: locale,
    ownerWindow: window,
    iframe: true,
    iframeSandbox: 'allow-same-origin',
    editHTMLDocumentMode: true,
    height: 400,
    iframeStyle: '',
    iframeTitle: '',
    // Only this authored-content frame is a security boundary. Its trusted host
    // must retain its real origin for Jodit's DOM access. Scripts/forms stay disabled here.
    iframeDoctype: `<!doctype html><meta data-ruphish-editor-policy="${policyMarker}" http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; img-src data: blob:; form-action 'none'; base-uri 'none'">`,
    sourceEditor: 'area',
    beautifyHTML: false,
    sourceEditorCDNUrlsJS: [],
    beautifyHTMLCDNUrlsJS: [],
    toolbarAdaptive: false,
    toolbarSticky: false,
    toolbarInline: false,
    buttons: [
      'bold',
      'italic',
      'underline',
      '|',
      'ul',
      'ol',
      '|',
      'fontsize',
      'paragraph',
      'brush',
      '|',
      'link',
      'image',
      'table',
      '|',
      'undo',
      'redo',
    ],
    statusbar: false,
    showPlaceholder: false,
    disablePlugins: [
      'source',
      'preview',
      'fullsize',
      'file',
      'video',
      'media',
      'image-properties',
      'image-processor',
      'drag-and-drop',
      'drag-and-drop-element',
      'paste-from-word',
      'paste-storage',
      'powered-by-jodit',
    ],
    cleanHTML: {
      // Preserve authored data. Isolation is supplied by the nested sandbox/CSP,
      // rather than silently deleting page behavior from the saved source.
      denyTags: false,
      removeEmptyElements: false,
      replaceOldTags: false,
      removeEventAttributes: false,
      removeOnError: false,
      safeJavaScriptLink: false,
      sandboxIframesInContent: false,
      convertUnsafeEmbeds: [],
      replaceNBSP: false,
    },
  };
}
