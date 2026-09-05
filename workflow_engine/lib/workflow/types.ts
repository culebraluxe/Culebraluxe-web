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
  /** Optional ordered node ids for presentation (e.g. portal timeline). */
  displayOrder?: string[];
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
    | 'dynamic-fork'
    | 'subprocess'
    | string;
  name?: string;
  description?: string;
  /**
   * Abstract responsibility hint (free string). The engine treats it as
   * presentation/resolution metadata and never resolves it. On task nodes it
   * is mirrored into `candidateGroups` for human-task candidate assignment.
   * The embedding application resolves hints to actual participants.
   */
  responsibility?: string;
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
  /**
   * Present on `dynamic-fork` nodes — data-driven N-way fan-out (ENG-FORGE-V9).
   * The engine reads the branch count from the process variable named by
   * `countVariable` (clamped to minimum..maximum), fans out that many branch
   * tokens, executes `branchCommandType` per branch, and rejoins at the node
   * id in `join` (fork-parent token correlation). `planVariable` is optional
   * structured per-branch intent produced by the caller (e.g. Lead's split plan).
   */
  countVariable?: string;
  planVariable?: string | null;
  branchCommandType?: string;
  join?: string;
  minimum?: number;
  maximum?: number;
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

/**
 * Test-only synchronization / fault-injection hooks. Throwing from any hook
 * aborts the enclosing engine step transaction, so the step rolls back as one
 * atomic unit (ENG-09). Generic - carries no domain semantics.
 */
export type EngineHooks = {
  /** In _arriveAtNode, before the node's handler / human-task creation. */
  beforeNodeArrive?: (nodeId: string) => void | Promise<void>;
  /** In _handleFork, before each child token insert. */
  beforeForkChildCreate?: (
    parentTokenId: string,
    toNodeId: string,
  ) => void | Promise<void>;
  /** In _handleJoin, after acquiring the fork-parent token lock (CRM-14B). */
  afterJoinParentLock?: (parentTokenId: string) => void | Promise<void>;
  /** In completeTask, after the task is marked completed, before token advance. */
  beforeTaskCompleteEvent?: (taskId: string) => void | Promise<void>;
  /** In fireTimerJob, after the job is completed, before the token move. */
  beforeTimerTokenMove?: (jobId: string) => void | Promise<void>;
  /** In _checkProcessCompletion / _terminateProcess, before the process update. */
  beforeProcessTerminal?: (processInstanceId: string) => void | Promise<void>;
  /** In _handleCommand, after app.executeCommand returns, before process_commands insert. */
  afterCommandSideEffect?: (commandId: string) => void | Promise<void>;
}

/**
 * Observer-only execution trace record emitted at workflow lifecycle points
 * (process started / node entered / transition taken / process terminal).
 * Carries no domain semantics and is never allowed to gate or abort a step.
 */
export type WorkflowTraceRecord = {
  eventType: string;
  system: string;
  occurredAt: string;
  outcome?: string | null;
  workflowInstanceId?: string | null;
  workflowDefinitionKey?: string | null;
  workflowDefinitionVersion?: number | null;
  workflowNodeId?: string | null;
  workflowTransitionId?: string | null;
  correlationId?: string | null;
  summary?: string | null;
  metadata?: Record<string, unknown> | null;
  /**
   * Business-context evidence derived from the workflow's subject, so a durable
   * trace event can answer "which deal / property / person was this about?"
   * without a read-time join. Nullable: not every workflow is deal/property/person
   * scoped and the recorder writes these only when the subject says so.
   */
  dealId?: string | null;
  propertyId?: string | null;
  personId?: string | null;
};

export interface EngineOptions {
  /** Decision-condition evaluator. Defaults to the expr-eval-backed evaluator. */
  evaluate?: ConditionEvaluator;
  /** Optional application command/fact bridge. */
  app?: ApplicationPort;
  /** Clock injection for deterministic tests. */
  now?: () => Date;
  /** Test-only synchronization / fault-injection hooks. */
  hooks?: EngineHooks;
  /**
   * Observer-only execution trace recorder. Called at lifecycle points with the
   * open step transaction as the executor (atomic with the step, and the
   * recorder itself must never throw — see `_trace` containment). Absent = no-op.
   */
  traceRecorder?: (input: WorkflowTraceRecord, execute: any) => void | Promise<void>;
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
