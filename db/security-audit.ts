import { sql } from './client'

// AUTH-02 durable security audit write service.
//
// Used only for security-significant success events (e.g. break-glass root
// login). Failed attempts are NOT persisted here (infrastructure log only).
// `metadata` must contain non-secret context only (never the secret/hash).

export async function recordSecurityAuditEvent(input: {
  appUserId: string | null
  eventType: string
  authenticationMethod: string
  metadata?: Record<string, unknown>
}): Promise<void> {
  await sql`
    insert into security_audit_event (
      app_user_id,
      event_type,
      authentication_method,
      metadata
    ) values (
      ${input.appUserId},
      ${input.eventType},
      ${input.authenticationMethod},
      ${JSON.stringify(input.metadata ?? {})}::jsonb
    )
  `
}
