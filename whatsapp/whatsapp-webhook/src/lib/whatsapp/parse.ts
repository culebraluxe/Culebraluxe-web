import { toE164 } from "./phone";
import type {
  NormalizedMessage,
  WhatsAppChangeValue,
  WhatsAppEcho,
  WhatsAppMessage,
  WhatsAppWebhookPayload,
} from "./types";

function extractBody(msg: WhatsAppMessage): string | null {
  if (msg.text?.body) return msg.text.body;
  if (msg.image?.caption) return msg.image.caption;
  if (msg.video?.caption) return msg.video.caption;
  if (msg.document?.caption) return msg.document.caption;
  if (msg.document?.filename) return msg.document.filename;
  if (msg.button?.text) return msg.button.text;
  if (msg.interactive?.button_reply?.title) return msg.interactive.button_reply.title;
  if (msg.interactive?.list_reply?.title) return msg.interactive.list_reply.title;
  return null;
}

function toDate(unixSeconds: string | undefined): Date {
  const n = Number(unixSeconds);
  if (!Number.isFinite(n) || n <= 0) return new Date();
  return new Date(n * 1000);
}

function normalizeInbound(msg: WhatsAppMessage): NormalizedMessage | null {
  const id = msg.id;
  const phone = toE164(msg.from);
  if (!id || !phone) return null;
  return {
    waMessageId: id,
    customerPhone: phone,
    direction: "in",
    occurredAt: toDate(msg.timestamp),
    messageType: msg.type ?? "unknown",
    body: extractBody(msg),
  };
}

function normalizeEcho(msg: WhatsAppEcho): NormalizedMessage | null {
  const id = msg.id;
  const phone = toE164(msg.to);
  if (!id || !phone) return null;
  return {
    waMessageId: id,
    customerPhone: phone,
    direction: "out",
    occurredAt: toDate(msg.timestamp),
    messageType: msg.type ?? "unknown",
    body: extractBody(msg),
  };
}

export function parseWebhook(payload: WhatsAppWebhookPayload, phoneNumberId?: string): NormalizedMessage[] {
  const out: NormalizedMessage[] = [];
  for (const entry of payload.entry ?? []) {
    for (const change of entry.changes ?? []) {
      const value = change.value;
      if (!value) continue;
      if (phoneNumberId && value.metadata?.phone_number_id && value.metadata.phone_number_id !== phoneNumberId) {
        continue;
      }
      collectFromValue(value, out);
    }
  }
  return out;
}

function collectFromValue(value: WhatsAppChangeValue, out: NormalizedMessage[]) {
  for (const msg of value.messages ?? []) {
    const n = normalizeInbound(msg);
    if (n) out.push(n);
  }
  for (const echo of value.message_echoes ?? []) {
    const n = normalizeEcho(echo);
    if (n) out.push(n);
  }
}
