export type JsonPrimitive = string | number | boolean | null

export type JsonValue =
  | JsonPrimitive
  | JsonValue[]
  | { [key: string]: JsonValue }

export type JsonObject = { [key: string]: JsonValue }

export type InteractionChannel =
  | 'website'
  | 'email'
  | 'call'
  | 'imessage'
  | 'sms'
  | 'calendar'
  | 'meeting'
  | 'showing'
  | 'document'
  | 'manual'
  | 'note'
  | 'whatsapp'

export type InteractionDirection = 'inbound' | 'outbound'

export interface Interaction {
  id: string
  personId: string
  propertyId?: string
  dealId?: string
  channel: InteractionChannel
  eventType: string
  direction?: InteractionDirection
  occurredAt: string
  title?: string
  summary?: string
  durationSeconds?: number
  sourceSystem?: string
  sourceExternalId?: string
  sourceMetadata: JsonObject
  createdAt: string
}

export interface CreateInteractionInput {
  personId: string
  propertyId?: string
  dealId?: string
  channel: InteractionChannel
  eventType: string
  direction?: InteractionDirection
  occurredAt: string | Date
  title?: string
  summary?: string
  durationSeconds?: number
  sourceSystem?: string
  sourceExternalId?: string
  sourceMetadata?: JsonObject
}

export type TaskKind = 'human' | 'system'
export type TaskStatus = 'open' | 'completed' | 'cancelled'

export interface Task {
  id: string
  title: string
  detail?: string
  personId?: string
  propertyId?: string
  dealId?: string
  sourceInteractionId?: string
  assignedUserId?: string
  dueAt?: string
  taskKind: TaskKind
  priority: number
  status: TaskStatus
  completedAt?: string
  createdAt: string
  updatedAt: string
}

export interface CreateTaskInput {
  title: string
  detail?: string
  personId?: string
  propertyId?: string
  dealId?: string
  sourceInteractionId?: string
  assignedUserId?: string
  dueAt?: string | Date
  taskKind?: TaskKind
  priority?: number
}
