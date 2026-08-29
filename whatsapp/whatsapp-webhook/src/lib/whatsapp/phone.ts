/** Normalize Meta wa_id / from / to into E.164 (+digits). */
export function toE164(raw: string | undefined | null): string | null {
  if (!raw) return null;
  const digits = raw.replace(/[^\d]/g, "");
  if (digits.length < 8) return null;
  return `+${digits}`;
}

export function previewText(body: string | null | undefined, max = 200): string | null {
  if (!body) return null;
  const trimmed = body.replace(/\s+/g, " ").trim();
  if (!trimmed) return null;
  return trimmed.length > max ? trimmed.slice(0, max) : trimmed;
}
