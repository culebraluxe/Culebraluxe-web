import type { AuthorizationDecision, ServiceErrorShape } from './types'

export class ServiceError extends Error {
  constructor(
    readonly code: string,
    message: string,
    readonly retryable = false,
    readonly cause?: unknown,
    /** Authorization decision that produced this denial (stamped on FORBIDDEN). */
    readonly authorization?: AuthorizationDecision,
  ) {
    super(message)
    this.name = 'ServiceError'
  }

  static from(cause: unknown): ServiceError {
    if (cause instanceof ServiceError) return cause
    if (cause instanceof Error) {
      return new ServiceError('UNEXPECTED', cause.message, false, cause)
    }
    return new ServiceError('UNEXPECTED', 'Unexpected service failure', false, cause)
  }

  /** Wire shape. Raw `cause` is intentionally NOT serialized — class-only. */
  toShape(): ServiceErrorShape {
    return {
      code: this.code,
      message: this.message,
      retryable: this.retryable,
    }
  }
}
