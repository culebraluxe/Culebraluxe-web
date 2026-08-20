// Deterministic engine errors. Kept small and generic — no brokerage semantics.

export class WorkflowError extends Error {
  constructor(message: string, readonly code: string) {
    super(message);
    this.name = 'WorkflowError';
  }
}

/** A deterministic conflict: retryable only by reconciling state, never silent. */
export class WorkflowConflictError extends WorkflowError {
  constructor(message: string, code = 'CONFLICT') {
    super(message, code);
    this.name = 'WorkflowConflictError';
  }
}

/** Optimistic-lock violation: the token moved/ended under the caller. */
export class StaleTokenError extends WorkflowConflictError {
  constructor(message: string) {
    super(message, 'STALE_TOKEN');
    this.name = 'StaleTokenError';
  }
}

/** The engine was asked to perform an application command without a port. */
export class MissingApplicationPortError extends WorkflowError {
  constructor(message: string) {
    super(message, 'MISSING_APPLICATION_PORT');
    this.name = 'MissingApplicationPortError';
  }
}
