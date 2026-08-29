import { neon } from "@neondatabase/serverless";
import { previewText } from "./phone";
import type { NormalizedMessage } from "./types";

function sqlClient() {
  const url = process.env.DATABASE_URL;
  if (!url) throw new Error("DATABASE_URL is not set");
  return neon(url);
}

/**
 * Default CRM table/column: contacts.phone (E.164).
 * If yours differ, change the update statement below — identifiers cannot
 * be bound as parameters.
 */
export async function persistMessages(messages: NormalizedMessage[]): Promise<number> {
  if (messages.length === 0) return 0;
  const sql = sqlClient();
  let stored = 0;

  for (const msg of messages) {
    const inserted = await sql`
      insert into whatsapp_events (
        wa_message_id, customer_phone, direction, occurred_at, message_type, body, raw
      ) values (
        ${msg.waMessageId},
        ${msg.customerPhone},
        ${msg.direction},
        ${msg.occurredAt.toISOString()},
        ${msg.messageType},
        ${msg.body},
        ${JSON.stringify(msg)}
      )
      on conflict (wa_message_id) do nothing
      returning wa_message_id
    `;

    if (!inserted.length) continue;
    stored += 1;

    const preview = previewText(msg.body);
    const at = msg.occurredAt.toISOString();

    await sql`
      update contacts
      set
        last_whatsapp_at = greatest(
          coalesce(last_whatsapp_at, '-infinity'::timestamptz),
          ${at}::timestamptz
        ),
        last_whatsapp_direction = case
          when last_whatsapp_at is null or ${at}::timestamptz >= last_whatsapp_at
          then ${msg.direction}
          else last_whatsapp_direction
        end,
        last_whatsapp_preview = case
          when last_whatsapp_at is null or ${at}::timestamptz >= last_whatsapp_at
          then ${preview}
          else last_whatsapp_preview
        end
      where phone = ${msg.customerPhone}
         or phone = ${msg.customerPhone.replace(/^\+/, "")}
    `;
  }

  return stored;
}
