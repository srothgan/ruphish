import {TemplateVault} from './template-html.mjs';

// Transform a copy only. No repository marker or new template syntax survives import.
export function prepareLanding(source, prefill, brandingJson, logo) {
  const branding = brandingJson ? JSON.parse(brandingJson) : null;
  if (!prefill && !branding) return source;
  const vault = new TemplateVault(source);
  const document = new DOMParser().parseFromString(vault.mask(source), 'text/html');
  if (prefill) {
    const fields = document.querySelectorAll('input[name="username"]');
    if (fields.length !== 1 || fields[0].type !== 'email') throw new Error('starter_invalid');
    fields[0].setAttribute('value', vault.mask('{{.Email | html}}'));
  }
  if (branding) {
    if (!/^data:image\/png;base64,[A-Za-z0-9+/]+=*$/.test(logo)) throw new Error('starter_logo_required');
    const targets = document.querySelectorAll(branding.selector);
    if (targets.length !== 1) throw new Error('starter_invalid');
    const target = targets[0];
    const image = document.createElement('img');
    image.src = logo;
    image.alt = 'Company logo';
    image.style.cssText = `display:block;width:auto;height:auto;max-width:100%;max-height:${branding.height}px;object-fit:contain`;
    if (target.tagName === 'IMG') {
      image.className = target.className;
      target.replaceWith(image);
    } else {
      // Remove the vendor wordmark as well as its icon, keeping the approved layout.
      target.replaceChildren(image);
    }
  }
  return '<!DOCTYPE html>\n' + vault.export(document);
}

export function validateLogoFile(file) {
  if (!file || !['image/png', 'image/jpeg', 'image/webp'].includes(file.type) ||
      file.size <= 0 || file.size > 512 * 1024) throw new Error('starter_logo_invalid');
}

export async function readLogo(input) {
  const file = input.files?.[0];
  validateLogoFile(file);
  const url = URL.createObjectURL(file);
  try {
    const image = new Image();
    image.src = url;
    await image.decode();
    if (!image.naturalWidth || !image.naturalHeight || image.naturalWidth > 4096 || image.naturalHeight > 4096)
      throw new Error('starter_logo_invalid');
    // Rasterize instead of trusting the extension/MIME, stripping metadata and active content.
    const scale = Math.min(1, 640 / image.naturalWidth, 240 / image.naturalHeight);
    const canvas = document.createElement('canvas');
    canvas.width = Math.max(1, Math.round(image.naturalWidth * scale));
    canvas.height = Math.max(1, Math.round(image.naturalHeight * scale));
    canvas.getContext('2d').drawImage(image, 0, 0, canvas.width, canvas.height);
    return canvas.toDataURL('image/png');
  } finally {
    URL.revokeObjectURL(url);
  }
}
