// Minimal uncompressed OOXML workbook for synthetic demo downloads only.
const encode = value => new TextEncoder().encode(value);
const xml = value => String(value ?? '').replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&apos;' })[c]);
const concat = parts => {
    const output = new Uint8Array(parts.reduce((size, part) => size + part.length, 0));
    let offset = 0;
    for (const part of parts) { output.set(part, offset); offset += part.length; }
    return output;
};
function crc32(bytes) {
    let crc = 0xffffffff;
    for (const byte of bytes) {
        crc ^= byte;
        for (let bit = 0; bit < 8; bit++) crc = (crc >>> 1) ^ ((crc & 1) ? 0xedb88320 : 0);
    }
    return (crc ^ 0xffffffff) >>> 0;
}
function zip(files) {
    const local = [], central = [];
    let offset = 0;
    for (const [path, content] of files) {
        const name = encode(path), body = encode(content), crc = crc32(body);
        const header = new Uint8Array(30), h = new DataView(header.buffer);
        h.setUint32(0, 0x04034b50, true); h.setUint16(4, 20, true);
        h.setUint32(14, crc, true); h.setUint32(18, body.length, true); h.setUint32(22, body.length, true); h.setUint16(26, name.length, true);
        local.push(header, name, body);
        const directory = new Uint8Array(46), d = new DataView(directory.buffer);
        d.setUint32(0, 0x02014b50, true); d.setUint16(4, 20, true); d.setUint16(6, 20, true);
        d.setUint32(16, crc, true); d.setUint32(20, body.length, true); d.setUint32(24, body.length, true); d.setUint16(28, name.length, true); d.setUint32(42, offset, true);
        central.push(directory, name);
        offset += header.length + name.length + body.length;
    }
    const directory = concat(central), end = new Uint8Array(22), e = new DataView(end.buffer);
    e.setUint32(0, 0x06054b50, true); e.setUint16(8, files.length, true); e.setUint16(10, files.length, true); e.setUint32(12, directory.length, true); e.setUint32(16, offset, true);
    return concat([...local, directory, end]);
}

export function workbook(sheets) {
    const ns = 'http://schemas.openxmlformats.org/';
    const files = [
        ['[Content_Types].xml', `<Types xmlns="${ns}package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>${sheets.map((_, i) => `<Override PartName="/xl/worksheets/sheet${i + 1}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>`).join('')}</Types>`],
        ['_rels/.rels', `<Relationships xmlns="${ns}package/2006/relationships"><Relationship Id="rId1" Type="${ns}officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>`],
        ['xl/workbook.xml', `<workbook xmlns="${ns}spreadsheetml/2006/main" xmlns:r="${ns}officeDocument/2006/relationships"><sheets>${sheets.map(([name], i) => `<sheet name="${xml(name)}" sheetId="${i + 1}" r:id="rId${i + 1}"/>`).join('')}</sheets></workbook>`],
        ['xl/_rels/workbook.xml.rels', `<Relationships xmlns="${ns}package/2006/relationships">${sheets.map((_, i) => `<Relationship Id="rId${i + 1}" Type="${ns}officeDocument/2006/relationships/worksheet" Target="worksheets/sheet${i + 1}.xml"/>`).join('')}</Relationships>`],
    ];
    for (const [index, [, rows]] of sheets.entries()) {
        const columns = Object.keys(rows[0] ?? { email: '' });
        const cells = [columns, ...rows.map(row => columns.map(key => row[key]))];
        files.push([`xl/worksheets/sheet${index + 1}.xml`, `<worksheet xmlns="${ns}spreadsheetml/2006/main"><sheetData>${cells.map(row => `<row>${row.map(value => typeof value === 'number' ? `<c><v>${value}</v></c>` : `<c t="inlineStr"><is><t xml:space="preserve">${xml(value)}</t></is></c>`).join('')}</row>`).join('')}</sheetData></worksheet>`]);
    }
    return zip(files);
}
