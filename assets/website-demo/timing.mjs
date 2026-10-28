// Pure, reproducible demo timing. The scenario calendar is Monday–Friday, 09–17 UTC.
export const DAY = 86400;
const HOUR = 3600;
const dayStart = (at) => Math.floor(at / DAY) * DAY;
const weekday = (at) => ![0, 6].includes(new Date(at * 1000).getUTCDay());

function businessDay(at, direction) {
  while (!weekday(at)) {
    at += direction * DAY;
  }
  return at;
}

export function campaignTiming(config, now) {
  const date = businessDay(dayStart(now) - config.ago * DAY, config.ago < 0 ? 1 : -1);
  const launch = date + config.launch_minute * 60;
  const sendBy = launch + config.send_minutes * 60;
  const end = businessDay(date + config.track_days * DAY, 1) + 17 * HOUR;
  return {
    launch,
    sendBy,
    end,
    status: now < launch ? 'Queued' : now >= end ? 'Completed' : 'In progress',
  };
}

export function addWorkingTime(at, seconds) {
  if (!Number.isSafeInteger(at) || !Number.isSafeInteger(seconds) || seconds < 0) {
    throw new Error('Invalid working-time interval');
  }
  for (;;) {
    const date = businessDay(dayStart(at), 1);
    at = Math.max(at, date + 9 * HOUR);
    const available = date + 17 * HOUR - at;
    if (available > 0 && seconds <= available) {
      return at + seconds;
    }
    seconds -= Math.max(0, available);
    at = date + DAY + 9 * HOUR;
  }
}

// FNV-1a key hashing followed by Mulberry32; synthetic variation, not cryptography.
function randomStream(key) {
  let state = 2166136261;
  for (const character of key) {
    state = Math.imul(state ^ character.charCodeAt(0), 16777619);
  }
  return () => {
    state = (state + 0x6d2b79f5) | 0;
    let value = Math.imul(state ^ (state >>> 15), 1 | state);
    value ^= value + Math.imul(value ^ (value >>> 7), 61 | value);
    return (((value ^ (value >>> 14)) >>> 0) + 0.5) / 4294967296;
  };
}

function shuffled(values, key) {
  const result = [...values];
  const random = randomStream(key);
  for (let i = result.length - 1; i > 0; i--) {
    const j = Math.floor(random() * (i + 1));
    [result[i], result[j]] = [result[j], result[i]];
  }
  return result;
}

export function recipientOutcomes(config, key) {
  const audience = Array.from({ length: config.size }, (_, n) => n);
  const opened = shuffled(audience, `${key}/openers`).slice(0, config.opened);
  const clicked = shuffled(opened, `${key}/clickers`).slice(0, config.clicked);
  const submitted = shuffled(clicked, `${key}/submitters`).slice(0, config.submitted);
  // Some people report after clicking; the rest report without a risky action.
  const overlap = Math.min(config.reported, Math.round(clicked.length * 0.2));
  const clickers = new Set(clicked);
  const reported = [
    ...shuffled(clicked, `${key}/late-reporters`).slice(0, overlap),
    ...shuffled(
      audience.filter((n) => !clickers.has(n)),
      `${key}/reporters`,
    ).slice(0, config.reported - overlap),
  ];
  return {
    opened: new Set(opened),
    clicked: clickers,
    submitted: new Set(submitted),
    reported: new Set(reported),
  };
}

function lognormal(random, median, sigma) {
  const normal = Math.sqrt(-2 * Math.log(random())) * Math.cos(2 * Math.PI * random());
  return Math.max(1, Math.round(median * Math.exp(sigma * normal)));
}

// Rejection sampling conditions required actions on closure without a spike at the boundary.
function requiredTime(random, parent, end, median, sigma, working = false) {
  for (let attempt = 0; attempt < 1000; attempt++) {
    const delay = lognormal(random, median, sigma);
    const at = working ? addWorkingTime(parent, delay) : parent + delay;
    if (at <= end) {
      return at;
    }
  }
  throw new Error('Demo interaction cannot fit before campaign closure');
}

export function recipientHistory({ key, sent, end, opened, clicked, submitted, reported }, now) {
  const events = [{ kind: 'Email Sent', at: sent, slot: 0 }];
  if (opened || clicked || submitted || reported) {
    // Reserve one second for each possible dependent action. Short session delays use wall time.
    const engagement = requiredTime(
      randomStream(`${key}/engagement`),
      sent,
      end - 3,
      45 * 60,
      1.8,
      true,
    );
    const add = (kind, at, slot) => events.push({ kind, at, slot });
    if (opened) {
      add('Email Opened', engagement, 10);
    }
    const click = clicked
      ? requiredTime(randomStream(`${key}/click`), engagement, end - 2, 90, 0.9)
      : null;
    if (click !== null) {
      add('Clicked Link', click, 20);
    }
    const submit = submitted
      ? requiredTime(randomStream(`${key}/submit`), click, end - 1, 45, 0.7)
      : null;
    if (submit !== null) {
      add('Submitted Data', submit, 30);
    }
    const report = reported
      ? requiredTime(
          randomStream(`${key}/report`),
          submit ?? click ?? engagement,
          end,
          clicked ? 15 * 60 : 3 * 60,
          1,
        )
      : null;
    if (report !== null) {
      add('Email Reported', report, 40);
    }
    for (const [kind, first, slot, probability, cap, median, sigma, working] of [
      ['Email Opened', opened ? engagement : null, 10, 0.35, 3, 45 * 60, 1.5, true],
      ['Clicked Link', click, 20, 0.15, 2, 8 * 60, 1.2, true],
      ['Submitted Data', submit, 30, 0.05, 1, 90, 0.8, false],
    ]) {
      if (first === null) {
        continue;
      }
      const random = randomStream(`${key}/${kind}/repeats`);
      let previous = first;
      for (let repeat = 1; repeat <= cap && random() < probability; repeat++) {
        const delay = lognormal(random, median, sigma);
        const at = working ? addWorkingTime(previous, delay) : previous + delay;
        // Once a recipient reports, stop risky actions; rereading the email is still possible.
        const deadline = kind === 'Email Opened' ? end : (report ?? end + 1) - 1;
        if (at > deadline) {
          break;
        }
        add(kind, at, slot + repeat);
        previous = at;
      }
    }
  }
  // Generate independently of the snapshot, then censor. IDs use slots, so future events never renumber history.
  return events.filter((e) => e.at < now).sort((a, b) => a.at - b.at || a.slot - b.slot);
}
