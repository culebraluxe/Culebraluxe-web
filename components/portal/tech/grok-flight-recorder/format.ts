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
  return new Intl.DateTimeFormat('en-US', {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
    hour: 'numeric',
    minute: '2-digit',
    second: '2-digit',
    fractionalSecondDigits: 3,
    hour12: true,
  }).format(new Date(iso));
}
