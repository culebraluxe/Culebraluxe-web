// AUTH-02 typed, deterministic authentication/authorization errors.
// No regex parsing of provider/database errors, no raw provider errors surfaced.

export type AuthErrorCode =
  | 'unauthenticated'
  | 'unmapped-identity'
  | 'inactive-account'
  | 'unauthorized'
  | 'missing-authority'

export class AuthError extends Error {
  readonly code: AuthErrorCode

  constructor(code: AuthErrorCode, message: string) {
    super(message)
    this.name = 'AuthError'
    this.code = code
  }
}

export class UnauthenticatedError extends AuthError {
  constructor(message = 'Not authenticated.') {
    super('unauthenticated', message)
    this.name = 'UnauthenticatedError'
  }
}

export class UnmappedIdentityError extends AuthError {
  constructor(provider: string, providerSubject: string) {
    super(
      'unmapped-identity',
      `No application user is mapped to this identity (${provider}).`,
    )
    this.name = 'UnmappedIdentityError'
    void providerSubject
  }
}

export class InactiveAccountError extends AuthError {
  constructor(message = 'This application account is inactive.') {
    super('inactive-account', message)
    this.name = 'InactiveAccountError'
  }
}

export class UnauthorizedError extends AuthError {
  constructor(message = 'Not authorized.') {
    super('unauthorized', message)
    this.name = 'UnauthorizedError'
  }
}

export class MissingAuthorityError extends AuthError {
  constructor(authority: string) {
    super('missing-authority', `Missing required authority: ${authority}`)
    this.name = 'MissingAuthorityError'
  }
}
