const DAY_MS = 24 * 60 * 60 * 1000;

function parseIsoDate(value) {
  if (!value) return null;
  const parts = String(value).split("-");
  if (parts.length !== 3) return null;
  const year = Number(parts[0]);
  const month = Number(parts[1]);
  const day = Number(parts[2]);
  if (!Number.isInteger(year) || !Number.isInteger(month) || !Number.isInteger(day)) {
    return null;
  }
  const date = new Date(Date.UTC(year, month - 1, day));
  if (
    date.getUTCFullYear() !== year ||
    date.getUTCMonth() !== month - 1 ||
    date.getUTCDate() !== day
  ) {
    return null;
  }
  return date;
}

function isoDate(date) {
  return date.toISOString().slice(0, 10);
}

function addDays(date, days) {
  return new Date(date.getTime() + days * DAY_MS);
}

function maxDate(left, right) {
  if (!left) return right;
  if (!right) return left;
  return left > right ? left : right;
}

function minDate(left, right) {
  if (!left) return right;
  if (!right) return left;
  return left < right ? left : right;
}

function boundedToday(todayIso, minIso, maxIso) {
  const today = parseIsoDate(todayIso) || new Date();
  return maxDate(minDate(today, parseIsoDate(maxIso)), parseIsoDate(minIso));
}

function boundedRange(from, to, minIso, maxIso) {
  const min = parseIsoDate(minIso);
  const max = parseIsoDate(maxIso);
  return {
    from: isoDate(maxDate(from, min)),
    to: isoDate(minDate(to, max)),
  };
}

export function rangeForDatePreset(preset, todayIso, minIso, maxIso) {
  const today = boundedToday(todayIso, minIso, maxIso);
  if (!today) return { from: "", to: "" };
  if (preset === "30") {
    return boundedRange(addDays(today, -30), today, minIso, maxIso);
  }
  if (preset === "90") {
    return boundedRange(addDays(today, -90), today, minIso, maxIso);
  }
  if (preset === "year") {
    return boundedRange(new Date(Date.UTC(today.getUTCFullYear(), 0, 1)), today, minIso, maxIso);
  }
  return { from: "", to: "" };
}

export function datePresetForRange(from, to, todayIso, minIso, maxIso) {
  if (!from && !to) return "all";
  for (const preset of ["30", "90", "year"]) {
    const range = rangeForDatePreset(preset, todayIso, minIso, maxIso);
    if (from === range.from && to === range.to) return preset;
  }
  return "custom";
}
