import type { ServiceErrorShape } from './types'

export class ServiceError extends Error {
  constructor(
    readonly code: string,
    message: string,
    readonly retryable = false,
    readonly cause?: unknown,
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

  toShape(): ServiceErrorShape {
    return {
      code: this.code,
      message: this.message,
      retryable: this.retryable,
      cause: this.cause,
    }
  }
}
