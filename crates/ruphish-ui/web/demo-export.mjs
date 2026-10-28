import { countValues, eventLabel, metrics } from './demo-reporting.mjs';
// Minimal uncompressed OOXML workbook for synthetic demo downloads only.
const encode = (value) => new TextEncoder().encode(value);
const xml = (value) =>
  String(value ?? '').replace(
    /[&<>"']/g,
    (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&apos;' })[c],
  );
const concat = (parts) => {
  const output = new Uint8Array(parts.reduce((size, part) => size + part.length, 0));
  let offset = 0;
  for (const part of parts) {
    output.set(part, offset);
    offset += part.length;
  }
  return output;
};
function crc32(bytes) {
  let crc = 0xffffffff;
  for (const byte of bytes) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit++) {
      crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0);
    }
  }
  return (crc ^ 0xffffffff) >>> 0;
}
function zip(files) {
  const local = [];
  const central = [];
  let offset = 0;
  for (const [path, content] of files) {
    const name = encode(path);
    const body = encode(content);
    const crc = crc32(body);
    const header = new Uint8Array(30);
    const h = new DataView(header.buffer);
    h.setUint32(0, 0x04034b50, true);
    h.setUint16(4, 20, true);
    h.setUint32(14, crc, true);
    h.setUint32(18, body.length, true);
    h.setUint32(22, body.length, true);
    h.setUint16(26, name.length, true);
    local.push(header, name, body);
    const directory = new Uint8Array(46);
    const d = new DataView(directory.buffer);
    d.setUint32(0, 0x02014b50, true);
    d.setUint16(4, 20, true);
    d.setUint16(6, 20, true);
    d.setUint32(16, crc, true);
    d.setUint32(20, body.length, true);
    d.setUint32(24, body.length, true);
    d.setUint16(28, name.length, true);
    d.setUint32(42, offset, true);
    central.push(directory, name);
    offset += header.length + name.length + body.length;
  }
  const directory = concat(central);
  const end = new Uint8Array(22);
  const e = new DataView(end.buffer);
  e.setUint32(0, 0x06054b50, true);
  e.setUint16(8, files.length, true);
  e.setUint16(10, files.length, true);
  e.setUint32(12, directory.length, true);
  e.setUint32(16, offset, true);
  return concat([...local, directory, end]);
}

export function workbook(sheets) {
  const ns = 'http://schemas.openxmlformats.org/';
  const files = [
    [
      '[Content_Types].xml',
      `<Types xmlns="${ns}package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>${sheets.map((_, i) => `<Override PartName="/xl/worksheets/sheet${i + 1}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>`).join('')}</Types>`,
    ],
    [
      '_rels/.rels',
      `<Relationships xmlns="${ns}package/2006/relationships"><Relationship Id="rId1" Type="${ns}officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>`,
    ],
    [
      'xl/workbook.xml',
      `<workbook xmlns="${ns}spreadsheetml/2006/main" xmlns:r="${ns}officeDocument/2006/relationships"><sheets>${sheets.map(([name], i) => `<sheet name="${xml(name)}" sheetId="${i + 1}" r:id="rId${i + 1}"/>`).join('')}</sheets></workbook>`,
    ],
    [
      'xl/_rels/workbook.xml.rels',
      `<Relationships xmlns="${ns}package/2006/relationships">${sheets.map((_, i) => `<Relationship Id="rId${i + 1}" Type="${ns}officeDocument/2006/relationships/worksheet" Target="worksheets/sheet${i + 1}.xml"/>`).join('')}</Relationships>`,
    ],
  ];
  for (const [index, [, rows]] of sheets.entries()) {
    const cells = rows;
    files.push([
      `xl/worksheets/sheet${index + 1}.xml`,
      `<worksheet xmlns="${ns}spreadsheetml/2006/main"><sheetData>${cells.map((row) => `<row>${row.map((value) => (typeof value === 'number' ? `<c><v>${value}</v></c>` : `<c t="inlineStr"><is><t xml:space="preserve">${xml(value)}</t></is></c>`)).join('')}</row>`).join('')}</sheetData></worksheet>`,
    ]);
  }
  return zip(files);
}

const recipientHeaders = [
  'Recipient ID',
  'Email',
  'Name',
  'Delivery',
  'Accepted events',
  'Open events',
  'Click events',
  'Submission events',
  'Report events',
  'Historical reported',
  'First Name',
  'Last Name',
  'Position',
  'Send Date (UTC)',
  'Modified Date (UTC)',
];
const eventHeaders = [
  'Event ID',
  'Recipient ID',
  'Email',
  'Time (UTC)',
  'Event',
  'Source',
  'Password present',
  'Edited',
];

function storedDate(value) {
  if (typeof value !== 'string') {
    return '';
  }
  const match = value.match(
    /^(\d{4}-\d{2}-\d{2})[ T](\d{2}:\d{2}:\d{2})(?:\.(\d{1,9}))?(Z|[+-]\d{2}:\d{2})$/,
  );
  if (!match) {
    return '';
  }
  const civil = `${match[1]}T${match[2]}`;
  const check = new Date(`${civil}Z`);
  const date = new Date(`${civil}${match[4]}`);
  if (
    !Number.isFinite(check.getTime()) ||
    check.toISOString().slice(0, 19) !== civil ||
    !Number.isFinite(date.getTime())
  ) {
    return '';
  }
  const fraction = (match[3] ?? '').replace(/0+$/, '');
  const utc = `${date.toISOString().slice(0, 19)}${fraction ? `.${fraction}` : ''}Z`;
  return utc === '0001-01-01T00:00:00Z' ? '' : utc;
}

const protectedText = (value) => {
  const text = String(value ?? '');
  return /^[ \t\r\n]*[=+@-]|^[\t\r\n]/.test(text) ? `'${text}` : text;
};

export function exportResponse(value, query) {
  const format = query.get('format') ?? 'csv';
  const scope = query.get('scope') ?? 'recipients';
  if (
    !['csv', 'xlsx', 'json'].includes(format) ||
    !['recipients', 'events', 'all'].includes(scope) ||
    (format === 'csv' && scope === 'all')
  ) {
    return new Response(
      JSON.stringify({
        success: false,
        code: 'DEMO_EXPORT_FORMAT',
        message: 'Invalid export format or scope',
      }),
      { status: 400, headers: { 'content-type': 'application/json' } },
    );
  }
  // Preserve the demo's existing JSON download contract.
  if (format === 'json') {
    const rows =
      scope === 'events'
        ? value.events.map((e) => ({
            email: e.email,
            time: e.time,
            event: eventLabel(e.kind),
            source: e.source,
            password_present: e.password_present,
          }))
        : value.recipients.map((r) => ({
            email: r.email,
            name: r.name,
            delivery: r.delivery,
            sent: r.counts.accepted,
            opened: r.counts.opened,
            clicked: r.counts.clicked,
            submitted: r.counts.submitted,
            reported: r.counts.reported,
          }));
    return new Response(JSON.stringify(scope === 'all' ? value : rows), {
      headers: { 'content-type': 'application/json', 'cache-control': 'no-store' },
    });
  }
  const recipients = [
    recipientHeaders,
    ...value.recipients.map((r) => [
      r.id,
      r.email,
      r.name,
      r.delivery ?? 'historical',
      ...countValues(r.counts),
      String(r.historical_reported),
      r.first_name ?? '',
      r.last_name ?? '',
      r.position ?? '',
      storedDate(r.send_date),
      storedDate(r.modified_date),
    ]),
  ];
  const events = [
    eventHeaders,
    ...value.events.map((e) => [
      e.id,
      e.recipient_id ?? '',
      e.email,
      e.time,
      eventLabel(e.kind),
      e.source,
      e.password_present === null ? '' : String(e.password_present),
      e.edited ?? false,
    ]),
  ];
  let bytes;
  let mime;
  if (format === 'csv') {
    const cell = (value) => `"${protectedText(value).replaceAll('"', '""')}"`;
    bytes = `${(scope === 'events' ? events : recipients).map((row) => row.map(cell).join(',')).join('\r\n')}\r\n`;
    mime = 'text/csv; charset=utf-8';
  } else {
    const summary = [
      ['Campaign', value.name],
      ['Campaign ID', value.id],
      ['Snapshot (UTC Unix seconds)', value.as_of],
      ['Audience', value.audience],
      ['From (inclusive UTC seconds)', query.has('from') ? Number(query.get('from')) : ''],
      ['Until (exclusive UTC seconds)', query.has('until') ? Number(query.get('until')) : ''],
      ['Recipient search', query.get('search') ?? ''],
      ['Selected recipient IDs', query.get('recipients') ?? query.get('recipient') ?? ''],
      ['Event filter', query.get('event') ?? ''],
      ['Source filter', query.get('source') ?? ''],
    ];
    for (const [index, name] of [
      'Accepted',
      'Opened',
      'Clicked',
      'Submitted',
      'Reported',
    ].entries()) {
      summary.push(
        [`${name} unique recipients`, value.unique[metrics[index]]],
        [`${name} total events`, value.totals[metrics[index]]],
      );
    }
    summary.push(
      ['Filtered event rows', value.event_total],
      ['Filtered recipient rows', value.recipient_total],
    );
    bytes = workbook([
      ['Summary', summary],
      ...(scope !== 'events' ? [['Recipients', recipients]] : []),
      ...(scope !== 'recipients' ? [['Events', events]] : []),
    ]);
    mime = 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet';
  }
  return new Response(bytes, {
    headers: {
      'content-type': mime,
      'content-disposition': `attachment; filename="campaign-${value.id}.${format}"`,
      'cache-control': 'no-store',
    },
  });
}
