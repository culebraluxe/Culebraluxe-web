/**
 * Meta WhatsApp Cloud API webhook (receive-only).
 * Callback URL: https://<your-prod-host>/api/whatsapp/webhook
 *
 * GET  — subscription handshake (hub.challenge as raw text)
 * POST — inbound + smb_message_echoes → Neon
 */
import { NextRequest } from "next/server";
import { persistMessages } from "@/lib/whatsapp/db";
import { parseWebhook } from "@/lib/whatsapp/parse";
import type { WhatsAppWebhookPayload } from "@/lib/whatsapp/types";
import { verifyHandshake, verifySignature } from "@/lib/whatsapp/verify";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

export async function GET(request: NextRequest) {
  const expected = process.env.WHATSAPP_VERIFY_TOKEN ?? "";
  const challenge = verifyHandshake({
    mode: request.nextUrl.searchParams.get("hub.mode"),
    token: request.nextUrl.searchParams.get("hub.verify_token"),
    challenge: request.nextUrl.searchParams.get("hub.challenge"),
    expectedToken: expected,
  });
  if (challenge == null) {
    return new Response("forbidden", { status: 403 });
  }
  return new Response(challenge, {
    status: 200,
    headers: { "content-type": "text/plain; charset=utf-8" },
  });
}

export async function POST(request: NextRequest) {
  const rawBody = await request.text();
  const secret = process.env.WHATSAPP_APP_SECRET ?? "";
  const signature = request.headers.get("x-hub-signature-256");

  if (!verifySignature(rawBody, signature, secret)) {
    return new Response("invalid signature", { status: 401 });
  }

  try {
    const payload = JSON.parse(rawBody) as WhatsAppWebhookPayload;
    const messages = parseWebhook(payload, process.env.WHATSAPP_PHONE_NUMBER_ID || undefined);
    await persistMessages(messages);
  } catch (err) {
    console.error("whatsapp webhook persist failed", err);
    // Still 200 after a valid signature so Meta does not disable the subscription
    // on a transient DB blip. Check Vercel logs + Neon if last_whatsapp_at stalls.
  }

  return new Response("ok", { status: 200 });
}
