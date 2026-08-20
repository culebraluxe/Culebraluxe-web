export type PortalWriteErrorCode = 'validation' | 'conflict' | 'not-found'

// Small typed domain error boundary for Portal write services. Server actions
// read `code` directly instead of classifying by message text; anything that
// is not a PortalWriteError falls back to 'unknown'.
export class PortalWriteError extends Error {
  readonly code: PortalWriteErrorCode

  constructor(code: PortalWriteErrorCode, message: string) {
    super(message)
    this.name = 'PortalWriteError'
    this.code = code
  }
}
