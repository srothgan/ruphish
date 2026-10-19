import test from 'node:test';
import assert from 'node:assert/strict';
import { analysisMinute, analysisTick } from '../analysis-time.mjs';
const seconds = value => Date.parse(value) / 1000;
const label = (value, step, first, last) => analysisTick(seconds(value), step, seconds(first), seconds(last), 'en-GB', 'Europe/Berlin');
test('short runs label minutes; week and month runs label dates without clock clutter', () => {
    assert.equal(label('2026-10-19T12:03:59Z',60,'2026-10-19T12:00Z','2026-10-19T12:05Z'), '14:03');
    const day = label('2026-10-19T12:00Z',86400,'2026-10-19T00:00Z','2026-10-26T00:00Z');
    assert.match(day,/19 Oct/); assert.doesNotMatch(day,/:/);
    const month = label('2026-10-19T12:00Z',432000,'2026-10-01T00:00Z','2026-10-31T00:00Z');
    assert.match(month,/19 Oct/); assert.doesNotMatch(month,/:/);
});
test('midnight, year transitions and repeated daylight-saving hours remain distinguishable', () => {
    assert.match(label('2026-10-19T22:30Z',3600,'2026-10-19T20:00Z','2026-10-20T04:00Z'),/20 Oct.*00:30/);
    assert.match(label('2027-01-01T12:00Z',86400,'2026-12-25T00:00Z','2027-01-05T00:00Z'),/2027/);
    const first='2026-10-25T00:00Z',last='2026-10-25T04:00Z';
    const before=label('2026-10-25T00:30Z',1800,first,last);
    const after=label('2026-10-25T01:30Z',1800,first,last);
    assert.match(before,/02:30/); assert.match(after,/02:30/); assert.notEqual(before,after);
});
test('minute presentation does not round an observation into the next minute', () => {
    assert.equal(analysisMinute('2026-10-19T12:03:59.999Z','en-GB','UTC'),analysisMinute('2026-10-19T12:03:00Z','en-GB','UTC'));
    assert.equal(analysisMinute('invalid','en-GB','UTC'),'Not recorded');
});
