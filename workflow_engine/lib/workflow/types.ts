// ---------------------------------------------------------------------------
// Process / token / task / job status enums
// ---------------------------------------------------------------------------

export type ProcessStatus =
  | 'active'
  | 'completed'
  | 'suspended'
  | 'aborted'
  | 'error';

/** Deliberate terminal disposition of a process instance. */
export type ProcessOutcome =
  | 'completed'
  | 'cancelled'
  | 'failed'
  | 'conflict';

export type TokenStatus = 'active' | 'completed' | 'suspended';

/** Runtime disposition of a token (and, for forked branches, of a branch). */
export type TokenOutcome =
  | 'completed'
  | 'cancelled'
  | 'failed'
  | 'skipped';

export type TaskStatus =
  | 'created'
  | 'ready'
  | 'reserved'
  | 'in_progress'
  | 'completed'
  | 'failed'
  | 'exited'
  | 'obsolete';

export type JobStatus =
  | 'pending'
  | 'locked'
  | 'completed'
  | 'failed'
  | 'cancelled';

export type JobType = 'timer' | 'async' | 'message' | 'signal';

// ---------------------------------------------------------------------------
// Definition-time metamodel
// ---------------------------------------------------------------------------

export interface ProcessDefinition {
  id: string;
  tenantId?: string | null;
  key: string;
  version: number;
  name: string;
  description?: string | null;
  definition: ProcessGraph;
  status: 'draft' | 'active' | 'deprecated';
  createdAt: Date;
  createdBy?: string | null;
  updatedAt: Date;
}

export interface ProcessGraph {
  nodes: Record<string, NodeDefinition>;
  startNodeId: string;
}

/** Minimal timer/deadline construct on a `timer` node. */
export interface TimerSpec {
  /** Absolute ISO-8601 timestamp. */
  dueAt?: string;
  /** Name of a process variable holding an ISO-8601 timestamp. */
  dueAtVariable?: string;
  /** Transition name to take when the timer fires (default: first transition). */
  transition?: string;
}

export interface NodeDefinition {
  id: string;
  type:
    | 'start'
    | 'end'
    | 'task'
    | 'decision'
    | 'fork'
    | 'join'
    | 'timer'
    | 'command'
    | 'state'
    | 'subprocess'
    | string;
  name?: string;
  description?: string;
  transitions?: TransitionDefinition[];
  formKey?: string;
  candidateGroups?: string[];
  priority?: number;
  decisions?: { condition: string; transition: string }[];
  subprocessKey?: string;
  inputMappings?: Record<string, any>;
  /**
   * End-node terminal outcome. Defaults to 'completed'. Other values
   * deliberately terminate the process with that outcome.
   */
  outcome?: ProcessOutcome;
  /** Present on `timer` nodes. */
  timer?: TimerSpec;
  /** Present on `command` nodes — stable machine command identifier. */
  commandType?: string;
  /** Present on `command` nodes — success transition name (default: first). */
  transition?: string;
  /** Present on `decision` nodes — refresh application facts before evaluating. */
  refreshFacts?: boolean;
}

export interface TransitionDefinition {
  name: string;
  to: string;
  condition?: string;
  /** Fork branch requirement. Default true. Optional branches never block a join. */
  required?: boolean;
}

// ---------------------------------------------------------------------------
// Runtime types
// ---------------------------------------------------------------------------

export interface ProcessInstance {
  id: string;
  tenantId?: string | null;
  definitionId: string;
  businessKey?: string | null;
  status: ProcessStatus;
  outcome?: ProcessOutcome | null;
  startedAt: Date;
  endedAt?: Date | null;
  startedBy?: string | null;
  parentInstanceId?: string | null;
  rootTokenId?: string | null;
  subjectType?: string | null;
  subjectId?: string | null;
  variables: Record<string, any>;
  version: number;
  createdAt: Date;
  updatedAt: Date;
}

export interface Token {
  id: string;
  tenantId?: string | null;
  processInstanceId: string;
  parentTokenId?: string | null;
  nodeId: string;
  status: TokenStatus;
  outcome?: TokenOutcome | null;
  required: boolean;
  isAbleToReactivateParent: boolean;
  startedAt: Date;
  endedAt?: Date | null;
  version: number;
  createdAt: Date;
  updatedAt: Date;
}

export interface Task {
  id: string;
  tenantId?: string | null;
  processInstanceId: string;
  tokenId?: string | null;
  name: string;
  description?: string | null;
  status: TaskStatus;
  assignee?: string | null;
  candidates: string[];
  swimlane?: string | null;
  priority: number;
  dueDate?: Date | null;
  formKey?: string | null;
  formData: Record<string, any>;
  createdAt: Date;
  claimedAt?: Date | null;
  completedAt?: Date | null;
  completedBy?: string | null;
  version: number;
  updatedAt: Date;
}

export interface Job {
  id: string;
  tenantId?: string | null;
  processInstanceId?: string | null;
  tokenId?: string | null;
  type: JobType | string;
  dueAt: Date;
  status: JobStatus;
  lockedBy?: string | null;
  lockedUntil?: Date | null;
  attempts: number;
  maxAttempts: number;
  payload: Record<string, any>;
  lastError?: string | null;
  createdAt: Date;
  updatedAt: Date;
  completedAt?: Date | null;
}

// ---------------------------------------------------------------------------
// Application integration seam (generic; the engine never imports CulebraLuxe)
// ---------------------------------------------------------------------------

export type ApplicationCommandOutcome =
  | 'success'
  | 'validation_failure'
  | 'not_found'
  | 'conflict'
  | 'unauthorized'
  | 'precondition_failure';

export interface WorkflowSubject {
  subjectType: string;
  subjectId: string;
}

export interface ApplicationCommandRequest {
  commandId: string;
  commandType: string;
  subjectType: string | null;
  subjectId: string | null;
  correlationId: string;
  causationId: string | null;
  input: Record<string, any>;
}

export interface ApplicationCommandResult {
  commandId: string;
  outcome: ApplicationCommandOutcome;
  message?: string | null;
  emittedEvents?: Record<string, unknown>[];
}

export type ApplicationFacts = Record<string, any>;

export interface ApplicationPort {
  executeCommand(
    request: ApplicationCommandRequest,
  ): Promise<ApplicationCommandResult>;
  readFacts(subject: WorkflowSubject): Promise<ApplicationFacts>;
}

export type ConditionEvaluator = (
  expression: string,
  variables: Record<string, any>,
) => boolean;

export interface EngineOptions {
  /** Decision-condition evaluator. Defaults to the expr-eval-backed evaluator. */
  evaluate?: ConditionEvaluator;
  /** Optional application command/fact bridge. */
  app?: ApplicationPort;
  /** Clock injection for deterministic tests. */
  now?: () => Date;
}

// ---------------------------------------------------------------------------
// Operation parameter types
// ---------------------------------------------------------------------------

export interface StartProcessParams {
  definitionKey: string;
  version?: number;
  businessKey?: string;
  variables?: Record<string, any>;
  startedBy: string;
  tenantId?: string;
  subject?: WorkflowSubject;
}

export interface SignalTokenParams {
  tokenId: string;
  transitionName?: string;
  signalName?: string;
  variables?: Record<string, any>;
  actor: string;
}

export interface CompleteTaskParams {
  taskId: string;
  userId: string;
  formData?: Record<string, any>;
  transitionName?: string;
}

export interface CancelProcessParams {
  processInstanceId: string;
  actor: string;
  reason?: string;
}

export interface FireTimerParams {
  jobId: string;
  workerId: string;
  variables?: Record<string, any>;
}

export interface CancelTimerParams {
  jobId: string;
  actor: string;
}

export interface RescheduleTimerParams {
  jobId: string;
  newDueAt: Date;
  actor: string;
}
