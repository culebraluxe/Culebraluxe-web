export type Direction = "in" | "out";

export type NormalizedMessage = {
  waMessageId: string;
  customerPhone: string;
  direction: Direction;
  occurredAt: Date;
  messageType: string;
  body: string | null;
};

export type WhatsAppChangeValue = {
  messaging_product?: string;
  metadata?: {
    display_phone_number?: string;
    phone_number_id?: string;
  };
  messages?: WhatsAppMessage[];
  message_echoes?: WhatsAppEcho[];
  contacts?: Array<{ wa_id?: string; profile?: { name?: string } }>;
  statuses?: unknown[];
};

export type WhatsAppMessage = {
  from?: string;
  to?: string;
  id?: string;
  timestamp?: string;
  type?: string;
  text?: { body?: string };
  image?: { caption?: string };
  video?: { caption?: string };
  document?: { caption?: string; filename?: string };
  button?: { text?: string };
  interactive?: { button_reply?: { title?: string }; list_reply?: { title?: string } };
};

export type WhatsAppEcho = WhatsAppMessage & {
  to?: string;
};

export type WhatsAppWebhookPayload = {
  object?: string;
  entry?: Array<{
    id?: string;
    changes?: Array<{
      field?: string;
      value?: WhatsAppChangeValue;
    }>;
  }>;
};
