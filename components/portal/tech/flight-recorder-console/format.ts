export function formatClock(iso: string): string {
  const d = new Date(iso);
  const pad = (n: number, w = 2) => String(n).padStart(w, '0');
  return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}.${pad(d.getMilliseconds(), 3)}`;
}

export function formatOffset(ms: number): string {
  const sign = ms < 0 ? '-' : '+';
  const abs = Math.abs(ms);
  if (abs < 1000) return `${sign}${abs}ms`;
  return `${sign}${(abs / 1000).toFixed(3).replace(/0+$/, '').replace(/\.$/, '')}s`;
}

export function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(3)}s`;
}

export function formatDisplayTime(iso: string): string {
  const d = new Date(iso)
  const base = {
    month: 'short' as const,
    day: 'numeric' as const,
    year: 'numeric' as const,
    hour: 'numeric' as const,
    minute: '2-digit' as const,
    second: '2-digit' as const,
    hour12: true,
  }
  try {
    // fractionalSecondDigits is only honored by engines that support it
    // (Chrome/Edge/Firefox 86+, Safari 16.4+). Guard so an older engine can't
    // throw a RangeError mid-render and break the console.
    return new Intl.DateTimeFormat('en-US', {
      ...base,
      fractionalSecondDigits: 3,
    }).format(d)
  } catch {
    return new Intl.DateTimeFormat('en-US', base).format(d)
  }
}
