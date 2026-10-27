// Reporting wire values shared by demo responses and tabular exports.
export const metrics = ['accepted', 'opened', 'clicked', 'submitted', 'reported'];
export const namedCounts = values => Object.fromEntries(metrics.map((name, i) => [name, values[i]]));
export const countValues = counts => metrics.map(name => counts[name]);
export const eventKinds = {
    'Campaign Created': 'campaign_created', 'Email Sent': 'email_sent',
    'Error Sending Email': 'email_error', 'Email Opened': 'email_opened',
    'Clicked Link': 'link_clicked', 'Submitted Data': 'form_submitted',
    'Email Reported': 'email_reported', 'Delivery reconciled': 'delivery_reconciled',
};
export const recordedKind = label => Object.hasOwn(eventKinds, label)
    ? { type: 'known', value: eventKinds[label] } : { type: 'historical', value: label };
export const eventLabel = kind => kind.type === 'historical' ? kind.value
    : Object.keys(eventKinds).find(label => eventKinds[label] === kind.value);
