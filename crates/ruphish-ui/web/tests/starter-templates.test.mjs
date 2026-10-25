import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFile, readdir} from 'node:fs/promises';
import {JSDOM} from 'jsdom';
import {preparePage, validateLogoFile, readLogo} from '../starter-templates.mjs';

const window = new JSDOM('').window;
globalThis.DOMParser = window.DOMParser;
const parse = source => new DOMParser().parseFromString(source, 'text/html');
const root = new URL('../../../ruphish-http/starter-templates/', import.meta.url);
const logo = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aZAAAAABJRU5ErkJggg==';
const branding = new Map([
  ['microsoft-full-screen', {selector:'.microsoft',height:36}],
  ['microsoft-side-panel', {selector:'.microsoft',height:36}],
  ['zoom', {selector:'.masthead',height:36}],
  ['salesforce', {selector:'img.logo',height:80}],
  ['workday', {selector:'.brand',height:48}],
]);

test('all landing pages remain offline; options default off and preserve independent content', async () => {
  const directories = await readdir(new URL('landing-pages/',root));
  assert.equal(directories.length,19);
  for (const slug of directories) {
    const source = await readFile(new URL(`landing-pages/${slug}/landing.html`,root),'utf8');
    assert.equal(preparePage(source,false,'',''),source);
    const standard = parse(source);
    assert.equal(standard.querySelector('input[type=email]').value,'');
    assert.equal(standard.querySelector('input[type=password]').value,'');
    assert.equal(standard.querySelector('input[type=password]').required,true);
    assert.equal(standard.querySelectorAll('script,iframe,object,embed,link').length,0);
    for (const image of standard.images) assert.match(image.src,/^data:image\/png;base64,/);
    const prefilled = preparePage(source,true,'','');
    assert.equal(parse(prefilled).querySelector('input[type=email]').value,'{{.Email | html}}');
    assert.equal(parse(prefilled).querySelector('input[type=password]').value,'');
    assert.equal(parse(source).querySelector('input[type=email]').value,'');
    // Only the five reviewed layouts allow this operation; unsupported entries have no option.
    if (branding.has(slug)) {
      const config = branding.get(slug);
      for (const prefill of [false,true]) {
        const prepared = parse(preparePage(source,prefill,JSON.stringify(config),logo));
        const target = prepared.querySelector(config.selector);
        const image = target.tagName === 'IMG' ? target : target.querySelector('img');
        assert.equal(image.src,logo);
        assert.equal(image.alt,'Company logo');
        assert.equal(image.style.maxHeight,`${config.height}px`);
        if (target.tagName !== 'IMG') assert.equal(target.textContent,'');
        assert.equal(prepared.querySelector('input[type=email]').value,prefill ? '{{.Email | html}}' : '');
        assert.equal(prepared.querySelector('input[type=password]').value,'');
        assert.equal(prepared.querySelectorAll('[data-starter-id],[data-catalog-id],[data-ruphish-field]').length,0);
        assert.equal(prepared.querySelectorAll('form').length,standard.querySelectorAll('form').length);
      }
      assert.throws(()=>preparePage(source,false,JSON.stringify(config),''),/starter_logo_required/);
      assert.throws(()=>preparePage(source,false,JSON.stringify(config),'https://example.test/logo.png'),/starter_logo_required/);
    }
  }
});

test('learning-page logos preserve static explanations and reject unusable branding without changing the source', () => {
  for (const slot of ['<div class="brand">Default mark</div>', '<img class="brand" alt="Default mark" src="data:image/png;base64,AAAA">']) {
    const source = `<!doctype html><html><body>${slot}<h1>Fixture explanation</h1><p>Literal {{.FirstName}} and {{ unfinished example</p></body></html>`;
    const metadata = JSON.stringify({selector:'.brand',height:48});
    assert.equal(preparePage(source,false,'','',true), source);
    const first = parse(preparePage(source,false,metadata,logo,true));
    const target = first.querySelector('.brand');
    const image = target.tagName === 'IMG' ? target : target.querySelector('img');
    assert.equal(image.src,logo);
    assert.equal(image.style.maxHeight,'48px');
    assert.equal(first.querySelector('h1').textContent,'Fixture explanation');
    assert.equal(first.querySelector('p').textContent,'Literal {{.FirstName}} and {{ unfinished example');
    assert.equal(first.querySelectorAll('form,input,[data-ruphish-field],[data-catalog-id]').length,0);
    const replacement = 'data:image/png;base64,BBBB';
    const second = parse(preparePage(source,false,metadata,replacement,true));
    assert.equal(second.querySelector('.brand img, img.brand').src,replacement);
    assert.equal(image.src,logo,'another copy retains its own embedded logo');
    assert.equal(parse(source).querySelector('.brand').outerHTML,parse(slot).querySelector('.brand').outerHTML);
    for (const invalid of ['', 'https://example.test/logo.png', 'data:image/svg+xml;base64,AAAA']) {
      assert.throws(()=>preparePage(source,false,metadata,invalid,true),/starter_logo_required/);
    }
    assert.throws(()=>preparePage(source,true,metadata,logo,true),/starter_invalid/);
    assert.throws(()=>preparePage(source,false,JSON.stringify({selector:'.missing',height:48}),logo,true),/starter_invalid/);
    assert.throws(()=>preparePage(source.replace('</body>','<div class="brand"></div></body>'),false,metadata,logo,true),/starter_invalid/);
  }
});

test('email catalog has no tracker or remote images and retains ordinary Go expressions', async () => {
  const directories = await readdir(new URL('emails/',root));
  assert.equal(directories.length,20);
  for (const slug of directories) {
    const source = await readFile(new URL(`emails/${slug}/email.html`,root),'utf8');
    assert.doesNotMatch(source,/\{\{\s*\.(Tracker|TrackingURL)/);
    assert.ok(/\{\{\.URL(?: \| html)?\}\}/.test(source), `${slug} has a campaign link`);
    const document = parse(source);
    assert.equal(document.querySelectorAll('script,iframe,object,embed,link').length,0);
    for (const image of document.images) assert.match(image.src,/^data:image\/png;base64,/);
  }
});

test('transformations fail closed on changed structures and preserve quoted Go syntax', () => {
  assert.throws(()=>preparePage('<input type=email>',true,'',''),/starter_invalid/);
  assert.throws(()=>preparePage('<input name=username type=email><input name=username type=email>',true,'',''),/starter_invalid/);
  assert.throws(()=>preparePage('<input name=username type=password>',true,'',''),/starter_invalid/);
  assert.throws(()=>preparePage('<div></div>',false,JSON.stringify({selector:'.missing',height:36}),logo),/starter_invalid/);
  const source = '<html><body><p>{{if eq .FirstName "A > B"}}Hello{{end}}</p><input name="username" type="email"></body></html>';
  assert.match(preparePage(source,true,'',''),/\{\{if eq \.FirstName "A > B"\}\}Hello\{\{end\}\}/);
});

test('upload validation rejects missing, oversized, empty and non-raster files', () => {
  for (const file of [null, {type:'image/png',size:0}, {type:'image/png',size:5242881},
    {type:'image/svg+xml',size:20}, {type:'text/html',size:20}, {type:'',size:20}]) {
    assert.throws(()=>validateLogoFile(file),/starter_logo_invalid/);
  }
  for (const type of ['image/png','image/jpeg','image/webp']) validateLogoFile({type,size:5242880});
});

test('logo decode errors and excessive dimensions never return content; object URLs are released', async () => {
  const original = {URL:globalThis.URL,Image:globalThis.Image,document:globalThis.document};
  const revoked = [];
  globalThis.URL = {createObjectURL:()=> 'blob:local',revokeObjectURL:url=>revoked.push(url)};
  try {
    const input = {files:[{type:'image/png',size:10}]};
    globalThis.Image = class { async decode() { throw new Error('decode rejected'); } };
    await assert.rejects(readLogo(input),/decode rejected/);
    globalThis.Image = class { naturalWidth=5000; naturalHeight=1; async decode() {} };
    await assert.rejects(readLogo(input),/starter_logo_invalid/);
    assert.deepEqual(revoked,['blob:local','blob:local']);
    let drawn;
    globalThis.Image = class { naturalWidth=1280; naturalHeight=480; async decode() {} };
    const canvas = {getContext:()=>({drawImage:(_image,_x,_y,w,h)=>{drawn=[w,h];}}),toDataURL:type=>{assert.equal(type,'image/png');return logo;}};
    globalThis.document = {createElement:tag=>{assert.equal(tag,'canvas');return canvas;}};
    assert.equal(await readLogo(input),logo);
    assert.deepEqual(drawn,[640,240]);
    assert.equal(revoked.length,3);
  } finally { Object.assign(globalThis,original); }
});
