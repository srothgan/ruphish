// In-memory CSV review for the static demo. Shared acceptance cases also run against the server.
// Resource writes remain simulated; no input is stored or sent over the network.
const bytes = (value) => new TextEncoder().encode(value).length;
const invalid = () => {
  throw new Error('CSV_INVALID');
};

function* records(text) {
  let fields = [];
  let value = '';
  let quoted = false;
  let started = false;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (quoted) {
      if (c === '"') {
        if (text[i + 1] === '"') {
          value += '"';
          i++;
        } else {
          quoted = false;
        }
      } else {
        value += c;
      }
    } else if (c === '"' && !started) {
      quoted = true;
      started = true;
    } else if (c === ',') {
      fields.push(value);
      value = '';
      started = false;
      if (fields.length > 1024) {
        invalid();
      }
    } else if (c === '\r' || c === '\n') {
      if (c === '\r' && text[i + 1] === '\n') {
        i++;
      }
      if (started || fields.length) {
        fields.push(value);
        yield fields;
      }
      fields = [];
      value = '';
      started = false;
    } else {
      value += c;
      started = true;
    }
  }
  // The production CSV reader accepts an unterminated final quoted field.
  if (started || fields.length) {
    fields.push(value);
    yield fields;
  }
}

function mailbox(value) {
  if (/[\r\n\0]/.test(value)) {
    return null;
  }
  const trimmed = value.trim();
  const bracketed = trimmed.match(/^(?:[^<>]*\s)?<([^<>]+)>$/);
  const email = bracketed ? bracketed[1] : trimmed;
  // Keep supplied spelling, including international mailboxes. Reject lists and header syntax.
  if (!/^[^\s<>(),;:"@]+@[^\s<>(),;:"@]+$/u.test(email)) {
    return null;
  }
  const [local, host] = email.split('@');
  if (
    bytes(local) > 64 ||
    bytes(host) > 255 ||
    local.startsWith('.') ||
    local.endsWith('.') ||
    local.includes('..')
  ) {
    return null;
  }
  if (
    host.split('.').some((label) => !/^[\p{L}\p{N}](?:[\p{L}\p{N}-]*[\p{L}\p{N}])?$/u.test(label))
  ) {
    return null;
  }
  return email;
}

export function reviewCsv({ content, columns }) {
  if (typeof content !== 'string' || bytes(content) > 8 * 1024 * 1024) {
    invalid();
  }
  const keys = columns ?? ['Email', 'First Name', 'Last Name', 'Position'];
  if (
    !Array.isArray(keys) ||
    keys.length !== 4 ||
    keys.some((k) => typeof k !== 'string' || bytes(k) > 1024)
  ) {
    invalid();
  }
  const rows = records(content.replace(/^\uFEFF/, ''));
  const headers = rows.next().value ?? [];
  if (headers.length > 1024) {
    invalid();
  }
  const normalize = (value) => value.trim().toLowerCase().replace(/[ _]/g, '');
  const names = headers.map(normalize);
  const indexes = keys.map((k) => names.indexOf(normalize(k)));
  if (indexes[0] < 0) {
    invalid();
  }
  const result = { targets: [], columns: headers, errors: [] };
  let row = 1;
  for (const fields of rows) {
    if (++row > 100001) {
      invalid();
    }
    if (fields.length !== headers.length) {
      result.errors.push({
        row,
        code: 'CSV_ROW_COLUMNS',
        message: 'Column count does not match the header',
        fields,
      });
      continue;
    }
    const values = indexes.map((index) => (fields[index] ?? '').trimStart());
    const email = mailbox(values[0]);
    if (email === null) {
      result.errors.push({ row, code: 'CSV_ROW_EMAIL', message: 'Invalid email address', fields });
    } else {
      result.targets.push({
        email,
        first_name: values[1],
        last_name: values[2],
        position: values[3],
      });
    }
  }
  return result;
}
