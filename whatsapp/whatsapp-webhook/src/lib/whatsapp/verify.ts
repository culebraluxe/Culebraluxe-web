import { createHmac, timingSafeEqual } from "node:crypto";

export function verifyHandshake(params: {
  mode: string | null;
  token: string | null;
  challenge: string | null;
  expectedToken: string;
}): string | null {
  if (params.mode !== "subscribe") return null;
  if (!params.token || !params.expectedToken) return null;
  if (params.token !== params.expectedToken) return null;
  return params.challenge;
}

/** Meta header is `sha256=<hex>`. Must hash the raw POST body. */
export function verifySignature(rawBody: string, header: string | null, appSecret: string): boolean {
  if (!header || !appSecret) return false;
  const incoming = header.startsWith("sha256=") ? header.slice("sha256=".length) : header;
  const expected = createHmac("sha256", appSecret).update(rawBody, "utf8").digest("hex");
  const a = Buffer.from(incoming, "utf8");
  const b = Buffer.from(expected, "utf8");
  if (a.length !== b.length) return false;
  return timingSafeEqual(a, b);
}
