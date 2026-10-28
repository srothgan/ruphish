// Typed demo ordering mirrors the public contract, with no DOM or locale-string parsing.
export function compare(a, b) {
  if (a == null || b == null) {
    return a == null ? (b == null ? 0 : -1) : 1;
  }
  if (typeof a === 'number' || typeof a === 'boolean') {
    return Number(a) - Number(b);
  }
  const left = Array.from(String(a), (c) => c.codePointAt(0));
  const right = Array.from(String(b), (c) => c.codePointAt(0));
  for (let i = 0; i < Math.min(left.length, right.length); i++) {
    if (left[i] !== right[i]) {
      return left[i] - right[i];
    }
  }
  return left.length - right.length;
}
export function selection(
  query,
  columns,
  sortKey = 'sort',
  orderKey = 'order',
  defaultColumn = columns[0],
  defaultOrder = 'asc',
) {
  if (query.getAll(sortKey).length > 1 || query.getAll(orderKey).length > 1) {
    throw new Error('Duplicate sort');
  }
  const column = query.get(sortKey) ?? defaultColumn;
  const order = query.get(orderKey) ?? defaultOrder;
  if (!columns.includes(column) || !['asc', 'desc'].includes(order)) {
    throw new Error('Invalid sort');
  }
  return { column, order };
}
export function ordered(rows, selected, value, identity = (row) => row.id) {
  return [...rows].sort(
    (a, b) =>
      compare(value(a, selected.column), value(b, selected.column)) *
        (selected.order === 'desc' ? -1 : 1) || compare(identity(a), identity(b)),
  );
}
const recipientColumns = [
  'email',
  'name',
  'delivery',
  'accepted',
  'opened',
  'clicked',
  'submitted',
  'reported',
];
export function recipientOrder(rows, query) {
  return ordered(rows, selection(query, recipientColumns), (row, key) =>
    recipientColumns.indexOf(key) >= 3 ? row.counts[recipientColumns.indexOf(key) - 3] : row[key],
  );
}
export function instantKey(value) {
  const parts = String(value ?? '').match(
    /^(\d{4}-\d{2}-\d{2}[Tt ]\d{2}:\d{2}:\d{2})(?:\.(\d{1,9}))?(Z|z|[+-]\d{2}:\d{2})?$/,
  );
  if (!parts || Number(parts[1].slice(0, 4)) < 1000) {
    return null;
  }
  const time = Date.parse(parts[1].replace(' ', 'T') + (parts[3] || 'Z'));
  if (!Number.isFinite(time)) {
    return null;
  }
  return new Date(time).toISOString().slice(0, 19) + '.' + (parts[2] ?? '').padEnd(9, '0');
}
export function eventOrder(rows, query) {
  const selected = selection(
    query,
    ['time', 'kind', 'source', 'password_present'],
    'event_sort',
    'event_order',
    'time',
    'desc',
  );
  // Retain the legacy descending identity order for equal event timestamps.
  return ordered(
    rows,
    selected,
    (row, key) => (key === 'time' ? instantKey(row.time) : row[key]),
    (row) => (selected.order === 'desc' ? -row.id : row.id),
  );
}
function position(row, column) {
  const value = column === 'launch' ? instantKey(row[column]) : row[column];
  return {
    present: value != null,
    text: typeof value === 'string' ? value : '',
    number: typeof value === 'number' ? value : 0,
    kind: row.kind === 'draft' ? 0 : 1,
    draft: row.kind === 'draft' ? row.id : '',
    id: row.kind === 'draft' ? 0 : Number(row.id),
  };
}
function positionCompare(a, b, order) {
  const values =
    compare(a.present, b.present) || compare(a.text, b.text) || compare(a.number, b.number);
  return (
    values * (order === 'desc' ? -1 : 1) ||
    compare(a.kind, b.kind) ||
    compare(a.draft, b.draft) ||
    compare(a.id, b.id)
  );
}
export function keyset(rows, query, columns, scope, limit, cursorKey = 'after') {
  const selected = selection(query, columns);
  if (query.getAll(cursorKey).length > 1) {
    throw new Error('Duplicate cursor');
  }
  const raw = query.get(cursorKey) ?? '';
  let after = null;
  if (raw) {
    if (new TextEncoder().encode(raw).length > 8192) {
      throw new Error('Invalid cursor');
    }
    const cursor = JSON.parse(raw);
    if (!cursor || Object.keys(cursor).sort().join(',') !== 'order,position,scope,sort,version') {
      throw new Error('Invalid cursor fields');
    }
    if (
      cursor.version !== 1 ||
      cursor.sort !== selected.column ||
      cursor.order !== selected.order ||
      cursor.scope !== scope
    ) {
      throw new Error('Wrong cursor selection');
    }
    after = cursor.position;
    if (!after || Object.keys(after).sort().join(',') !== 'draft,id,kind,number,present,text') {
      throw new Error('Invalid position fields');
    }
    if (
      !after ||
      typeof after.present !== 'boolean' ||
      typeof after.text !== 'string' ||
      new TextEncoder().encode(after.text).length > 4096 ||
      typeof after.draft !== 'string' ||
      after.draft.length > 64 ||
      ![0, 1].includes(after.kind) ||
      !Number.isSafeInteger(after.number) ||
      !Number.isSafeInteger(after.id) ||
      after.id < 0
    ) {
      throw new Error('Invalid position');
    }
  }
  const keyed = rows.map((row) => ({ row, key: position(row, selected.column) }));
  keyed.sort((a, b) => positionCompare(a.key, b.key, selected.order));
  const matches = keyed.filter(
    (item) => !after || positionCompare(item.key, after, selected.order) > 0,
  );
  const items = matches.slice(0, limit);
  const cursor =
    matches.length > limit
      ? JSON.stringify({
          version: 1,
          sort: selected.column,
          order: selected.order,
          scope,
          position: items.at(-1).key,
        })
      : null;
  return { items: items.map((item) => item.row), cursor };
}
