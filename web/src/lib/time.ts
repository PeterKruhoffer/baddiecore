/** Formats Unix seconds in the viewer's locale and time zone. */
export function formatTime(seconds: number) {
  return new Date(seconds * 1000).toLocaleString(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  });
}

/**
 * Converts a `datetime-local` value, read in the viewer's time zone, to Unix seconds.
 * Returns undefined for invalid values and for times skipped by a daylight-saving
 * change, which `Date` would silently move an hour. Repeated times use the first one.
 */
export function localInputSeconds(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime()) || localInputValue(date) !== value) return undefined;
  return Math.floor(date.getTime() / 1000);
}

/** The `datetime-local` value for a time, in the viewer's time zone. */
export function localInputValue(date: Date) {
  const local = new Date(date.getTime() - date.getTimezoneOffset() * 60_000);
  return local.toISOString().slice(0, 16);
}

export const timeZone = Intl.DateTimeFormat().resolvedOptions().timeZone;
