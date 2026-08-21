import { createHash } from 'node:crypto';
import {
  ProcessDefinition,
  ProcessGraph,
  ProcessInstance,
  Token,
  Task,
  Job,
  NodeDefinition,
  TransitionDefinition,
  StartProcessParams,
  SignalTokenParams,
  CompleteTaskParams,
  CancelProcessParams,
  FireTimerParams,
  CancelTimerParams,
  RescheduleTimerParams,
  ProcessStatus,
  ProcessOutcome,
  TokenOutcome,
  EngineOptions,
  EngineHooks,
  ApplicationCommandRequest,
  ApplicationCommandResult,
} from './types';
import { evaluateCondition } from './expressions';
import {
  StaleTokenError,
  WorkflowConflictError,
  MissingApplicationPortError,
} from './errors';

type SqlClient = any; // neon tagged template client

export class WorkflowEngine {
  private evaluate: (
    expression: string,
    variables: Record<string, any>,
  ) => boolean;
  private app?: EngineOptions['app'];
  private now: () => Date;
  private hooks?: EngineHooks;

  constructor(private sql: SqlClient, options: EngineOptions = {}) {
    this.evaluate = options.evaluate ?? evaluateCondition;
    this.app = options.app;
    this.now = options.now ?? (() => new Date());
    this.hooks = options.hooks;
  }

  // ----------------------------------------------------------------
  // startProcess
  // ----------------------------------------------------------------
  async startProcess(params: StartProcessParams): Promise<{
    processInstanceId: string;
    rootTokenId: string;
  }> {
    const {
      definitionKey,
      version,
      businessKey,
      variables = {},
      startedBy,
      tenantId,
      subject,
    } = params;

    return await this.sql.begin(async (tx: SqlClient) => {
      const definition = await this._loadDefinition(tx, definitionKey, version, tenantId);
      const graph = definition.definition as ProcessGraph;

      if (!graph.startNodeId || !graph.nodes[graph.startNodeId]) {
        throw new Error('Invalid process definition: missing start node');
      }

      const instanceRows = await tx`
        INSERT INTO process_instances (
          tenant_id, definition_id, business_key, status, started_by, variables,
          subject_type, subject_id
        ) VALUES (
          ${tenantId ?? null},
          ${definition.id},
          ${businessKey ?? null},
          'active',
          ${startedBy},
          ${JSON.stringify(variables)},
          ${subject?.subjectType ?? null},
          ${subject?.subjectId ?? null}
        )
        RETURNING id
      `;
      const processInstanceId = instanceRows[0].id as string;

      const tokenRows = await tx`
        INSERT INTO tokens (
          tenant_id, process_instance_id, node_id, status, required
        ) VALUES (
          ${tenantId ?? null},
          ${processInstanceId},
          ${graph.startNodeId},
          'active',
          true
        )
        RETURNING id
      `;
      const rootTokenId = tokenRows[0].id as string;

      await tx`
        UPDATE process_instances
        SET root_token_id = ${rootTokenId}
        WHERE id = ${processInstanceId}
      `;

      await this._event(tx, {
        tenantId: tenantId ?? null,
        processInstanceId,
        tokenId: rootTokenId,
        eventType: 'process.started',
        nodeId: graph.startNodeId,
        actor: startedBy,
        data: {
          definitionKey: definition.key,
          definitionVersion: definition.version,
          businessKey,
          variables,
        },
      });

      // Leave start node
      await this._executeNodeLeave(tx, {
        token: {
          id: rootTokenId,
          processInstanceId,
          nodeId: graph.startNodeId,
          status: 'active',
          version: 1,
          tenantId: tenantId ?? null,
          required: true,
        } as Token,
        instance: {
          id: processInstanceId,
          definitionId: definition.id,
          status: 'active',
          variables,
          tenantId: tenantId ?? null,
          version: 1,
          subjectType: subject?.subjectType ?? null,
          subjectId: subject?.subjectId ?? null,
        } as ProcessInstance,
        graph,
        actor: startedBy,
        variables,
      });

      return { processInstanceId, rootTokenId };
    });
  }

  // ----------------------------------------------------------------
  // signalToken
  // ----------------------------------------------------------------
  async signalToken(params: SignalTokenParams): Promise<void> {
    const { tokenId, transitionName, variables = {}, actor } = params;

    await this.sql.begin(async (tx: SqlClient) => {
      const tokenRows = await tx`SELECT * FROM tokens WHERE id = ${tokenId} FOR UPDATE`;
      const token = this._mapToken(tokenRows[0]);
      if (!token) throw new Error(`Token not found: ${tokenId}`);
      if (token.status !== 'active') {
        throw new Error(`Token ${tokenId} is not active`);
      }

      const instanceRows = await tx`
        SELECT * FROM process_instances WHERE id = ${token.processInstanceId} FOR UPDATE
      `;
      const instance = this._mapInstance(instanceRows[0]);
      if (instance.status !== 'active') {
        throw new Error('Process instance is not active');
      }

      const defRows = await tx`
        SELECT * FROM process_definitions WHERE id = ${instance.definitionId}
      `;
      const definition = this._mapDefinition(defRows[0]);
      const graph = definition.definition as ProcessGraph;

      let currentVariables = instance.variables;
      if (Object.keys(variables).length > 0) {
        currentVariables = { ...currentVariables, ...variables };
        await tx`
          UPDATE process_instances
          SET variables = ${JSON.stringify(currentVariables)},
              version = version + 1
          WHERE id = ${instance.id}
        `;
      }

      await this._executeNodeLeave(tx, {
        token,
        instance: { ...instance, variables: currentVariables },
        graph,
        actor,
        preferredTransition: transitionName,
        variables: currentVariables,
      });
    });
  }

  // ----------------------------------------------------------------
  // Task methods
  // ----------------------------------------------------------------
  async claimTask(taskId: string, userId: string): Promise<void> {
    await this.sql.begin(async (tx: SqlClient) => {
      const taskRows = await tx`SELECT * FROM tasks WHERE id = ${taskId} FOR UPDATE`;
      const task = this._mapTask(taskRows[0]);
      if (!task) throw new Error(`Task not found: ${taskId}`);

      if (task.status !== 'ready' && task.status !== 'reserved') {
        throw new Error(`Task cannot be claimed in status: ${task.status}`);
      }

      const canClaim =
        task.assignee === userId ||
        task.candidates.includes(userId) ||
        task.candidates.length === 0;

      if (!canClaim) {
        throw new Error(`User ${userId} is not allowed to claim this task`);
      }
      if (task.assignee && task.assignee !== userId) {
        throw new Error(`Task is already claimed by ${task.assignee}`);
      }

      await tx`
        UPDATE tasks
        SET status = 'reserved',
            assignee = ${userId},
            claimed_at = ${this.now()},
            version = version + 1
        WHERE id = ${taskId} AND version = ${task.version}
      `;

      await this._event(tx, {
        tenantId: task.tenantId,
        processInstanceId: task.processInstanceId,
        tokenId: task.tokenId,
        taskId,
        eventType: 'task.claimed',
        actor: userId,
        data: { previousStatus: task.status },
      });
    });
  }

  async releaseTask(taskId: string, userId: string): Promise<void> {
    await this.sql.begin(async (tx: SqlClient) => {
      const taskRows = await tx`SELECT * FROM tasks WHERE id = ${taskId} FOR UPDATE`;
      const task = this._mapTask(taskRows[0]);
      if (!task) throw new Error(`Task not found: ${taskId}`);
      if (task.assignee !== userId) {
        throw new Error('Only the assignee can release the task');
      }
      if (task.status !== 'reserved' && task.status !== 'in_progress') {
        throw new Error(`Task cannot be released in status: ${task.status}`);
      }

      await tx`
        UPDATE tasks
        SET status = 'ready',
            assignee = null,
            claimed_at = null,
            version = version + 1
        WHERE id = ${taskId} AND version = ${task.version}
      `;

      await this._event(tx, {
        tenantId: task.tenantId,
        processInstanceId: task.processInstanceId,
        tokenId: task.tokenId,
        taskId,
        eventType: 'task.released',
        actor: userId,
        data: {},
      });
    });
  }

  async completeTask(params: CompleteTaskParams): Promise<void> {
    const { taskId, userId, formData = {}, transitionName } = params;

    await this.sql.begin(async (tx: SqlClient) => {
      const taskRows = await tx`SELECT * FROM tasks WHERE id = ${taskId} FOR UPDATE`;
      const task = this._mapTask(taskRows[0]);
      if (!task) throw new Error(`Task not found: ${taskId}`);

      if (!['ready', 'reserved', 'in_progress'].includes(task.status)) {
        throw new Error(`Task cannot be completed in status: ${task.status}`);
      }
      if (task.assignee && task.assignee !== userId) {
        throw new Error(`Task is assigned to ${task.assignee}`);
      }

      await tx`
        UPDATE tasks
        SET status = 'completed',
            assignee = ${userId},
            form_data = ${JSON.stringify({ ...task.formData, ...formData })},
            completed_at = ${this.now()},
            completed_by = ${userId},
            version = version + 1
        WHERE id = ${taskId} AND version = ${task.version}
      `;

      await this._event(tx, {
        tenantId: task.tenantId,
        processInstanceId: task.processInstanceId,
        tokenId: task.tokenId,
        taskId,
        eventType: 'task.completed',
        actor: userId,
        data: { formData, transitionName },
      });

      if (task.tokenId) {
        const tokenRows = await tx`SELECT * FROM tokens WHERE id = ${task.tokenId} FOR UPDATE`;
        const token = this._mapToken(tokenRows[0]);
        if (!token || token.status !== 'active') {
          throw new Error('Linked token is not active');
        }

        const instanceRows = await tx`
          SELECT * FROM process_instances WHERE id = ${task.processInstanceId} FOR UPDATE
        `;
        const instance = this._mapInstance(instanceRows[0]);

        const defRows = await tx`
          SELECT * FROM process_definitions WHERE id = ${instance.definitionId}
        `;
        const definition = this._mapDefinition(defRows[0]);
        const graph = definition.definition as ProcessGraph;

        const newVariables = {
          ...instance.variables,
          ...formData,
          [`task_${task.name}_result`]: formData,
        };

        await tx`
          UPDATE process_instances
          SET variables = ${JSON.stringify(newVariables)},
              version = version + 1
          WHERE id = ${instance.id}
        `;

        await this._executeNodeLeave(tx, {
          token,
          instance: { ...instance, variables: newVariables },
          graph,
          actor: userId,
          preferredTransition: transitionName,
          variables: newVariables,
        });
      }
    });
  }

  // ----------------------------------------------------------------
  // Process cancellation
  // ----------------------------------------------------------------
  async cancelProcess(params: CancelProcessParams): Promise<void> {
    const { processInstanceId, actor, reason } = params;

    await this.sql.begin(async (tx: SqlClient) => {
      const rows = await tx`
        SELECT * FROM process_instances WHERE id = ${processInstanceId} FOR UPDATE
      `;
      const instance = this._mapInstance(rows[0]);
      if (!instance) throw new Error(`Process not found: ${processInstanceId}`);

      if (instance.status !== 'active') {
        if (instance.outcome === 'cancelled') {
          // Idempotent: already cancelled.
          return;
        }
        throw new WorkflowConflictError(
          `Process ${processInstanceId} is not active (status=${instance.status}, outcome=${instance.outcome ?? 'none'})`,
          'PROCESS_NOT_ACTIVE',
        );
      }

      await this._terminateProcess(tx, processInstanceId, actor, 'cancelled', reason);
    });
  }

  // ----------------------------------------------------------------
  // Jobs
  // ----------------------------------------------------------------
  async claimJobs(workerId: string, limit = 10): Promise<Job[]> {
    const now = this.now();
    const lockUntil = new Date(now.getTime() + 5 * 60 * 1000);
    const rows = await this.sql`
      UPDATE jobs
      SET status = 'locked',
          locked_by = ${workerId},
          locked_until = ${lockUntil},
          attempts = attempts + 1,
          updated_at = ${now}
      WHERE id IN (
        SELECT id FROM jobs
        WHERE status = 'pending'
          AND due_at <= ${now}
          AND attempts < max_attempts
        ORDER BY due_at ASC
        LIMIT ${limit}
        FOR UPDATE SKIP LOCKED
      )
      RETURNING *
    `;
    return rows.map((r: any) => this._mapJob(r));
  }

  async completeJob(jobId: string, workerId: string): Promise<void> {
    await this.sql.begin(async (tx: SqlClient) => {
      const jobRows = await tx`SELECT * FROM jobs WHERE id = ${jobId} FOR UPDATE`;
      const job = this._mapJob(jobRows[0]);
      if (!job) throw new Error(`Job not found: ${jobId}`);
      if (job.lockedBy !== workerId) {
        throw new Error('Job is not locked by this worker');
      }

      await tx`
        UPDATE jobs
        SET status = 'completed',
            completed_at = ${this.now()},
            locked_by = null,
            locked_until = null
        WHERE id = ${jobId}
      `;

      if (job.processInstanceId) {
        await this._event(tx, {
          tenantId: job.tenantId,
          processInstanceId: job.processInstanceId,
          tokenId: job.tokenId,
          jobId,
          eventType: 'job.completed',
          actor: workerId,
          data: { type: job.type },
        });
      }
    });
  }

  async failJob(jobId: string, workerId: string, error: string): Promise<void> {
    await this.sql.begin(async (tx: SqlClient) => {
      const jobRows = await tx`SELECT * FROM jobs WHERE id = ${jobId} FOR UPDATE`;
      const job = this._mapJob(jobRows[0]);
      if (!job) throw new Error(`Job not found: ${jobId}`);
      if (job.lockedBy !== workerId) {
        throw new Error('Job is not locked by this worker');
      }

      const shouldRetry = job.attempts < job.maxAttempts;
      const retryDueAt = new Date(this.now().getTime() + 60_000 * Math.pow(2, job.attempts));

      await tx`
        UPDATE jobs
        SET status = ${shouldRetry ? 'pending' : 'failed'},
            last_error = ${error},
            locked_by = null,
            locked_until = null,
            due_at = ${shouldRetry ? retryDueAt : job.dueAt}
        WHERE id = ${jobId}
      `;

      if (job.processInstanceId) {
        await this._event(tx, {
          tenantId: job.tenantId,
          processInstanceId: job.processInstanceId,
          tokenId: job.tokenId,
          jobId,
          eventType: shouldRetry ? 'job.retry_scheduled' : 'job.failed',
          actor: workerId,
          data: { error, attempts: job.attempts },
        });
      }
    });
  }

  async createJob(params: {
    processInstanceId?: string;
    tokenId?: string;
    tenantId?: string | null;
    type: string;
    dueAt: Date;
    payload?: Record<string, any>;
    maxAttempts?: number;
  }): Promise<string> {
    const rows = await this.sql`
      INSERT INTO jobs (
        tenant_id, process_instance_id, token_id, type, due_at, payload, max_attempts
      ) VALUES (
        ${params.tenantId ?? null},
        ${params.processInstanceId ?? null},
        ${params.tokenId ?? null},
        ${params.type},
        ${params.dueAt},
        ${JSON.stringify(params.payload ?? {})},
        ${params.maxAttempts ?? 5}
      )
      RETURNING id
    `;
    return rows[0].id as string;
  }

  // ----------------------------------------------------------------
  // Timer job lifecycle (definition-bound timers)
  // ----------------------------------------------------------------
  async fireTimerJob(params: FireTimerParams): Promise<void> {
    const { jobId, workerId, variables = {} } = params;

    await this.sql.begin(async (tx: SqlClient) => {
      const jobRows = await tx`SELECT * FROM jobs WHERE id = ${jobId} FOR UPDATE`;
      const job = this._mapJob(jobRows[0]);
      if (!job) throw new Error(`Job not found: ${jobId}`);
      if (job.status === 'completed') {
        throw new WorkflowConflictError(
          `Timer job ${jobId} already fired`,
          'TIMER_ALREADY_FIRED',
        );
      }
      if (job.status !== 'locked') {
        throw new WorkflowConflictError(
          `Timer job ${jobId} is not locked (status=${job.status})`,
          'TIMER_NOT_LOCKED',
        );
      }
      if (job.lockedBy !== workerId) {
        throw new WorkflowConflictError(
          `Timer job ${jobId} is locked by another worker`,
          'TIMER_LOCK_OWNER',
        );
      }

      await tx`
        UPDATE jobs
        SET status = 'completed',
            completed_at = ${this.now()},
            locked_by = null,
            locked_until = null
        WHERE id = ${jobId}
      `;

      if (job.tokenId) {
        const tokenRows = await tx`SELECT * FROM tokens WHERE id = ${job.tokenId} FOR UPDATE`;
        const token = this._mapToken(tokenRows[0]);
        if (token && token.status === 'active') {
          const instanceRows = await tx`
            SELECT * FROM process_instances WHERE id = ${token.processInstanceId} FOR UPDATE
          `;
          const instance = this._mapInstance(instanceRows[0]);
          if (instance && instance.status === 'active') {
            const defRows = await tx`
              SELECT * FROM process_definitions WHERE id = ${instance.definitionId}
            `;
            const definition = this._mapDefinition(defRows[0]);
            const graph = definition.definition as ProcessGraph;
            const node = graph.nodes[token.nodeId];

            let currentVariables = instance.variables;
            if (Object.keys(variables).length > 0) {
              currentVariables = { ...currentVariables, ...variables };
              await tx`
                UPDATE process_instances
                SET variables = ${JSON.stringify(currentVariables)},
                    version = version + 1
                WHERE id = ${instance.id}
              `;
            }

            const timer = node?.timer;
            const transitionName = timer?.transition ?? node?.transitions?.[0]?.name;
            const transition =
              node?.transitions?.find((t: TransitionDefinition) => t.name === transitionName) ??
              node?.transitions?.[0];
            if (!transition) {
              throw new Error(`Timer node ${token.nodeId} has no resume transition`);
            }

            await this._moveToken(tx, token, transition.to, transition.name, workerId);
            const fresh = this._mapToken(
              (await tx`SELECT * FROM tokens WHERE id = ${token.id}`)[0],
            );
            await this._arriveAtNode(tx, {
              token: fresh!,
              instance: { ...instance, variables: currentVariables },
              graph,
              actor: workerId,
              variables: currentVariables,
            });
          }
        }
      }

      if (job.processInstanceId) {
        await this._event(tx, {
          tenantId: job.tenantId,
          processInstanceId: job.processInstanceId,
          tokenId: job.tokenId,
          jobId,
          eventType: 'timer.fired',
          actor: workerId,
          data: { type: job.type },
        });
      }
    });
  }

  async cancelTimerJob(params: CancelTimerParams): Promise<void> {
    const { jobId, actor } = params;

    await this.sql.begin(async (tx: SqlClient) => {
      const jobRows = await tx`SELECT * FROM jobs WHERE id = ${jobId} FOR UPDATE`;
      const job = this._mapJob(jobRows[0]);
      if (!job) throw new Error(`Job not found: ${jobId}`);
      if (job.status === 'cancelled') return; // idempotent
      if (job.status !== 'pending') {
        throw new WorkflowConflictError(
          `Job ${jobId} cannot be cancelled (status=${job.status})`,
          'JOB_NOT_CANCELLABLE',
        );
      }

      await tx`UPDATE jobs SET status = 'cancelled' WHERE id = ${jobId}`;

      if (job.processInstanceId) {
        await this._event(tx, {
          tenantId: job.tenantId,
          processInstanceId: job.processInstanceId,
          tokenId: job.tokenId,
          jobId,
          eventType: 'timer.cancelled',
          actor,
          data: { type: job.type },
        });
      }
    });
  }

  async rescheduleTimerJob(params: RescheduleTimerParams): Promise<void> {
    const { jobId, newDueAt, actor } = params;

    await this.sql.begin(async (tx: SqlClient) => {
      const jobRows = await tx`SELECT * FROM jobs WHERE id = ${jobId} FOR UPDATE`;
      const job = this._mapJob(jobRows[0]);
      if (!job) throw new Error(`Job not found: ${jobId}`);
      if (job.status !== 'pending') {
        throw new WorkflowConflictError(
          `Job ${jobId} cannot be rescheduled (status=${job.status})`,
          'JOB_NOT_RESCHEDULABLE',
        );
      }

      await tx`UPDATE jobs SET due_at = ${newDueAt}, updated_at = ${this.now()} WHERE id = ${jobId}`;

      if (job.processInstanceId) {
        await this._event(tx, {
          tenantId: job.tenantId,
          processInstanceId: job.processInstanceId,
          tokenId: job.tokenId,
          jobId,
          eventType: 'timer.rescheduled',
          actor,
          data: { newDueAt: newDueAt.toISOString() },
        });
      }
    });
  }

  // ----------------------------------------------------------------
  // Read methods
  // ----------------------------------------------------------------
  async getProcessInstance(id: string): Promise<ProcessInstance | null> {
    const rows = await this.sql`SELECT * FROM process_instances WHERE id = ${id}`;
    return rows[0] ? this._mapInstance(rows[0]) : null;
  }

  async getToken(id: string): Promise<Token | null> {
    const rows = await this.sql`SELECT * FROM tokens WHERE id = ${id}`;
    return rows[0] ? this._mapToken(rows[0]) : null;
  }

  async getTask(id: string): Promise<Task | null> {
    const rows = await this.sql`SELECT * FROM tasks WHERE id = ${id}`;
    return rows[0] ? this._mapTask(rows[0]) : null;
  }

  async getActiveTasksForUser(userId: string, tenantId?: string): Promise<Task[]> {
    const rows = await this.sql`
      SELECT * FROM tasks
      WHERE status IN ('ready', 'reserved', 'in_progress')
        AND (assignee = ${userId} OR ${userId} = ANY(candidates))
        ${tenantId ? this.sql`AND tenant_id = ${tenantId}` : this.sql``}
      ORDER BY priority DESC, created_at ASC
    `;
    return rows.map((r: any) => this._mapTask(r));
  }

  async getProcessInstanceWithDetails(id: string) {
    const instance = await this.getProcessInstance(id);
    if (!instance) return null;

    const [tokens, tasks, definition] = await Promise.all([
      this.sql`SELECT * FROM tokens WHERE process_instance_id = ${id} ORDER BY created_at`,
      this.sql`SELECT * FROM tasks WHERE process_instance_id = ${id} ORDER BY created_at`,
      this.sql`SELECT id, key, version, name, status FROM process_definitions WHERE id = ${instance.definitionId}`,
    ]);

    return {
      ...instance,
      definition: definition[0] ?? null,
      tokens: tokens.map((t: any) => this._mapToken(t)),
      tasks: tasks.map((t: any) => this._mapTask(t)),
    };
  }

  async getProcessHistory(processInstanceId: string, limit = 100) {
    return await this.sql`
      SELECT * FROM process_events
      WHERE process_instance_id = ${processInstanceId}
      ORDER BY created_at DESC, id DESC
      LIMIT ${limit}
    `;
  }

  async findProcessInstances(params: {
    tenantId?: string;
    status?: ProcessStatus | ProcessStatus[];
    definitionKey?: string;
    businessKey?: string;
    limit?: number;
    offset?: number;
  }) {
    const { tenantId, status, definitionKey, businessKey, limit = 50, offset = 0 } = params;

    const rows = await this.sql`
      SELECT pi.*
      FROM process_instances pi
      JOIN process_definitions pd ON pd.id = pi.definition_id
      WHERE 1=1
        ${tenantId ? this.sql`AND pi.tenant_id = ${tenantId}` : this.sql``}
        ${
          status
            ? Array.isArray(status)
              ? this.sql`AND pi.status = ANY(${status})`
              : this.sql`AND pi.status = ${status}`
            : this.sql``
        }
        ${definitionKey ? this.sql`AND pd.key = ${definitionKey}` : this.sql``}
        ${businessKey ? this.sql`AND pi.business_key = ${businessKey}` : this.sql``}
      ORDER BY pi.started_at DESC
      LIMIT ${limit}
      OFFSET ${offset}
    `;
    return rows.map((r: any) => this._mapInstance(r));
  }

  // ----------------------------------------------------------------
  // Internal execution
  // ----------------------------------------------------------------
  private async _arriveAtNode(
    tx: SqlClient,
    opts: {
      token: Token;
      instance: ProcessInstance;
      graph: ProcessGraph;
      actor: string;
      preferredTransition?: string;
      variables: Record<string, any>;
    },
  ) {
    const node = opts.graph.nodes[opts.token.nodeId];
    if (!node) throw new Error(`Node ${opts.token.nodeId} not found`);
    if (node.type === 'task') {
      // A token placed/moved onto a task node waits at the human gate.
      await this._createHumanTask(tx, {
        instance: opts.instance,
        tokenId: opts.token.id,
        node,
        actor: opts.actor,
      });
      return;
    }
    await this._executeNodeLeave(tx, opts);
  }

  private async _executeNodeLeave(
    tx: SqlClient,
    opts: {
      token: Token;
      instance: ProcessInstance;
      graph: ProcessGraph;
      actor: string;
      preferredTransition?: string;
      variables: Record<string, any>;
    },
  ) {
    const { token, instance, graph, actor, preferredTransition, variables } = opts;
    const node = graph.nodes[token.nodeId];
    if (!node) throw new Error(`Node ${token.nodeId} not found`);

    // Terminal: end node (with optional outcome) or a leaf with nowhere to go.
    if (
      node.type === 'end' ||
      (!node.transitions?.length && node.type !== 'timer' && node.type !== 'command')
    ) {
      const endOutcome: ProcessOutcome =
        node.type === 'end' ? (node.outcome ?? 'completed') : 'completed';
      await this._completeToken(tx, token, actor, this._tokenOutcomeForEnd(endOutcome));
      await this._resolveProcessAfterToken(tx, instance.id, actor, token, endOutcome);
      return;
    }

    if (node.type === 'timer') {
      await this._handleTimer(tx, token, node, instance, actor, variables);
      return;
    }

    if (node.type === 'command') {
      await this._handleCommand(tx, token, node, instance, graph, actor, variables);
      return;
    }

    if (node.type === 'decision') {
      let decisionVariables = variables;
      if (node.refreshFacts !== false && this.app) {
        decisionVariables = await this._refreshFacts(tx, instance, variables);
      }
      const chosen = this._evaluateDecision(node, decisionVariables, preferredTransition);
      if (!chosen) throw new Error(`No valid transition from decision node ${node.id}`);
      await this._moveToken(tx, token, chosen.to, chosen.name, actor);
      const fresh = this._mapToken(
        (await tx`SELECT * FROM tokens WHERE id = ${token.id}`)[0],
      );
      // Consume the preferred transition — it applied only to the node we just
      // left; a downstream decision must re-evaluate, not inherit a stale name.
      await this._arriveAtNode(tx, {
        ...opts,
        token: fresh!,
        variables: decisionVariables,
        preferredTransition: undefined,
      });
      return;
    }

    if (node.type === 'fork') {
      await this._handleFork(tx, token, node, instance, graph, actor, variables);
      return;
    }

    if (node.type === 'join') {
      await this._handleJoin(tx, token, node, instance, graph, actor, variables);
      return;
    }

    let transition = node.transitions?.[0];
    if (preferredTransition) {
      transition = node.transitions?.find((t) => t.name === preferredTransition);
    }
    if (!transition) {
      throw new Error(`No transition found from node ${node.id}`);
    }

    await this._moveToken(tx, token, transition.to, transition.name, actor);

    const fresh = this._mapToken(
      (await tx`SELECT * FROM tokens WHERE id = ${token.id}`)[0],
    );
    // Consume the preferred transition — it applied only to the node we just
    // left; a downstream node must not inherit a stale name.
    await this._arriveAtNode(tx, { ...opts, token: fresh!, preferredTransition: undefined });
  }

  private async _moveToken(
    tx: SqlClient,
    token: Token,
    toNodeId: string,
    transitionName: string,
    actor: string,
  ) {
    const moved = await tx`
      UPDATE tokens
      SET node_id = ${toNodeId},
          version = version + 1
      WHERE id = ${token.id} AND version = ${token.version}
      RETURNING id
    `;
    if (!moved[0]) {
      throw new StaleTokenError(
        `Token ${token.id} moved concurrently (expected version ${token.version})`,
      );
    }

    await this._event(tx, {
      tenantId: token.tenantId,
      processInstanceId: token.processInstanceId,
      tokenId: token.id,
      eventType: 'token.moved',
      nodeId: toNodeId,
      actor,
      data: { from: token.nodeId, transition: transitionName },
    });
  }

  private async _completeToken(
    tx: SqlClient,
    token: Token,
    actor: string,
    outcome: TokenOutcome = 'completed',
  ) {
    await tx`
      UPDATE tokens
      SET status = 'completed',
          ended_at = ${this.now()},
          outcome = ${outcome},
          version = version + 1
      WHERE id = ${token.id}
    `;

    await this._event(tx, {
      tenantId: token.tenantId,
      processInstanceId: token.processInstanceId,
      tokenId: token.id,
      eventType: 'token.completed',
      nodeId: token.nodeId,
      actor,
      data: { outcome },
    });
  }

  private async _checkProcessCompletion(
    tx: SqlClient,
    processInstanceId: string,
    actor: string,
  ) {
    const active = await tx`
      SELECT count(*)::int AS cnt FROM tokens
      WHERE process_instance_id = ${processInstanceId} AND status = 'active'
    `;

    if (active[0].cnt === 0) {
      await tx`
        UPDATE process_instances
        SET status = 'completed',
            outcome = 'completed',
            ended_at = ${this.now()},
            version = version + 1
        WHERE id = ${processInstanceId}
      `;

      await this._event(tx, {
        processInstanceId,
        eventType: 'process.completed',
        actor,
        data: { outcome: 'completed' },
      });
    }
  }

  private async _terminateProcess(
    tx: SqlClient,
    processInstanceId: string,
    actor: string,
    outcome: ProcessOutcome,
    reason?: string,
  ) {
    const status: ProcessStatus =
      outcome === 'cancelled'
        ? 'aborted'
        : outcome === 'completed'
          ? 'completed'
          : 'error';

    await tx`
      UPDATE process_instances
      SET status = ${status},
          outcome = ${outcome},
          ended_at = ${this.now()},
          version = version + 1
      WHERE id = ${processInstanceId}
    `;

    // Close every remaining active token as cancelled.
    const activeTokens = await tx`
      SELECT * FROM tokens
      WHERE process_instance_id = ${processInstanceId} AND status = 'active'
    `;
    for (const row of activeTokens) {
      await tx`
        UPDATE tokens
        SET status = 'completed',
            ended_at = ${this.now()},
            outcome = 'cancelled',
            version = version + 1
        WHERE id = ${row.id}
      `;
      await this._event(tx, {
        tenantId: row.tenant_id,
        processInstanceId,
        tokenId: row.id,
        eventType: 'token.cancelled',
        nodeId: row.node_id,
        actor,
        data: { reason: reason ?? null },
      });
    }

    // Obsolete open human tasks.
    const openTasks = await tx`
      SELECT id FROM tasks
      WHERE process_instance_id = ${processInstanceId}
        AND status IN ('ready', 'reserved', 'in_progress')
    `;
    for (const t of openTasks) {
      await tx`UPDATE tasks SET status = 'obsolete', version = version + 1 WHERE id = ${t.id}`;
      await this._event(tx, {
        processInstanceId,
        taskId: t.id,
        eventType: 'task.obsoleted',
        actor,
        data: { reason: reason ?? null },
      });
    }

    // Cancel open jobs.
    const openJobs = await tx`
      SELECT id FROM jobs
      WHERE process_instance_id = ${processInstanceId}
        AND status IN ('pending', 'locked')
    `;
    for (const j of openJobs) {
      await tx`
        UPDATE jobs SET status = 'cancelled', locked_by = null, locked_until = null
        WHERE id = ${j.id}
      `;
    }

    const eventType =
      outcome === 'cancelled'
        ? 'process.cancelled'
        : outcome === 'failed'
          ? 'process.failed'
          : 'process.conflict';

    await this._event(tx, {
      processInstanceId,
      eventType,
      actor,
      data: { outcome, reason: reason ?? null },
    });
  }

  private async _resolveProcessAfterToken(
    tx: SqlClient,
    processInstanceId: string,
    actor: string,
    token: Token,
    endOutcome: ProcessOutcome,
  ) {
    if (endOutcome === 'completed') {
      await this._checkProcessCompletion(tx, processInstanceId, actor);
      return;
    }
    if (token.required === false) {
      // Optional branch: record the disposition but never terminate the process.
      await this._checkProcessCompletion(tx, processInstanceId, actor);
      return;
    }
    await this._terminateProcess(
      tx,
      processInstanceId,
      actor,
      endOutcome,
      `end node outcome: ${endOutcome}`,
    );
  }

  private _tokenOutcomeForEnd(endOutcome: ProcessOutcome): TokenOutcome {
    switch (endOutcome) {
      case 'completed':
        return 'completed';
      case 'cancelled':
        return 'cancelled';
      case 'failed':
        return 'failed';
      case 'conflict':
        return 'failed';
    }
  }

  private async _createHumanTask(
    tx: SqlClient,
    opts: {
      instance: ProcessInstance;
      tokenId: string;
      node: NodeDefinition;
      actor: string;
    },
  ) {
    const { instance, tokenId, node, actor } = opts;
    const candidates = node.candidateGroups ?? [];

    const taskRows = await tx`
      INSERT INTO tasks (
        tenant_id, process_instance_id, token_id, name, description,
        status, candidates, form_key, priority
      ) VALUES (
        ${instance.tenantId},
        ${instance.id},
        ${tokenId},
        ${node.name ?? node.id},
        ${node.description ?? null},
        'ready',
        ${candidates},
        ${node.formKey ?? null},
        ${node.priority ?? 0}
      )
      RETURNING id
    `;

    await this._event(tx, {
      processInstanceId: instance.id,
      tokenId,
      taskId: taskRows[0].id,
      eventType: 'task.created',
      nodeId: node.id,
      actor,
      data: { name: node.name, candidates },
    });
  }

  private _evaluateDecision(
    node: NodeDefinition,
    variables: Record<string, any>,
    preferredTransition?: string,
  ): TransitionDefinition | null {
    if (preferredTransition) {
      return node.transitions?.find((t) => t.name === preferredTransition) ?? null;
    }

    for (const d of node.decisions ?? []) {
      if (this.evaluate(d.condition, variables)) {
        return node.transitions?.find((t) => t.name === d.transition) ?? null;
      }
    }

    return node.transitions?.[0] ?? null;
  }

  private async _handleFork(
    tx: SqlClient,
    parentToken: Token,
    node: NodeDefinition,
    instance: ProcessInstance,
    graph: ProcessGraph,
    actor: string,
    variables: Record<string, any>,
  ) {
    await this._completeToken(tx, parentToken, actor, 'completed');

    for (const transition of node.transitions ?? []) {
      // A prior branch may have terminated the process; do not spawn further.
      const liveRows = await tx`
        SELECT * FROM process_instances WHERE id = ${parentToken.processInstanceId} FOR UPDATE
      `;
      const live = this._mapInstance(liveRows[0]);
      if (live.status !== 'active') break;

      const required = transition.required !== false;
      const childRows = await tx`
        INSERT INTO tokens (
          tenant_id, process_instance_id, parent_token_id, node_id, status, required
        ) VALUES (
          ${parentToken.tenantId},
          ${parentToken.processInstanceId},
          ${parentToken.id},
          ${transition.to},
          'active',
          ${required}
        )
        RETURNING *
      `;
      const childToken = this._mapToken(childRows[0])!;

      await this._event(tx, {
        processInstanceId: instance.id,
        tokenId: childToken.id,
        eventType: 'token.forked',
        nodeId: transition.to,
        actor,
        data: { parentTokenId: parentToken.id, transition: transition.name, required },
      });

      await this._arriveAtNode(tx, {
        token: childToken,
        instance,
        graph,
        actor,
        variables,
      });
    }
  }

  private async _handleJoin(
    tx: SqlClient,
    token: Token,
    node: NodeDefinition,
    instance: ProcessInstance,
    graph: ProcessGraph,
    actor: string,
    variables: Record<string, any>,
  ) {
    const parentId = token.parentTokenId;
    if (!parentId) {
      // Join without a fork parent: degenerate passthrough.
      const transition = node.transitions?.[0];
      if (transition) {
        const rows = await tx`
          INSERT INTO tokens (
            tenant_id, process_instance_id, node_id, status, required
          ) VALUES (
            ${token.tenantId},
            ${token.processInstanceId},
            ${transition.to},
            'active',
            true
          )
          RETURNING *
        `;
        const newToken = this._mapToken(rows[0])!;
        await this._arriveAtNode(tx, { token: newToken, instance, graph, actor, variables });
      }
      return;
    }

    // Serialize the join release on the fork parent token (CRM-14B).
    // Both branches of a fork share the same parent_token_id, so locking
    // the parent row means exactly one branch transaction decides the
    // release at a time: the winner completes its branch and either waits
    // or releases; the loser blocks on this lock and re-reads the sibling
    // count AFTER the winner commits. Exactly-once release - no stuck join,
    // no duplicate successor token.
    await tx`SELECT id FROM tokens WHERE id = ${parentId} FOR UPDATE`;
    if (this.hooks?.afterJoinParentLock) {
      await this.hooks.afterJoinParentLock(parentId);
    }

    await this._completeToken(tx, token, actor, 'completed');

    const requiredActive = await tx`
      SELECT count(*)::int AS cnt
      FROM tokens
      WHERE parent_token_id = ${parentId} AND status = 'active' AND required = true
    `;
    if (requiredActive[0].cnt > 0) return; // still waiting on required branches

    // All required branches are done. Skip any still-active optional branches.
    const optionalActive = await tx`
      SELECT * FROM tokens
      WHERE parent_token_id = ${parentId} AND status = 'active' AND required = false
    `;
    for (const row of optionalActive) {
      await tx`
        UPDATE tokens
        SET status = 'completed',
            ended_at = ${this.now()},
            outcome = 'skipped',
            version = version + 1
        WHERE id = ${row.id}
      `;
      await this._event(tx, {
        processInstanceId: instance.id,
        tokenId: row.id,
        eventType: 'token.skipped',
        nodeId: row.node_id,
        actor,
        data: {},
      });

      // Close any open human task still anchored to the skipped branch.
      const openTasks = await tx`
        SELECT id FROM tasks
        WHERE token_id = ${row.id} AND status IN ('ready', 'reserved', 'in_progress')
      `;
      for (const t of openTasks) {
        await tx`UPDATE tasks SET status = 'obsolete', version = version + 1 WHERE id = ${t.id}`;
        await this._event(tx, {
          processInstanceId: instance.id,
          taskId: t.id,
          eventType: 'task.obsoleted',
          actor,
          data: { reason: 'branch skipped' },
        });
      }

      // Cancel any open job still anchored to the skipped branch. A timer (or
      // other job) whose token can no longer legally execute must not remain
      // operationally pending.
      const openJobs = await tx`
        SELECT * FROM jobs
        WHERE token_id = ${row.id} AND status IN ('pending', 'locked')
      `;
      for (const j of openJobs) {
        await tx`
          UPDATE jobs
          SET status = 'cancelled', locked_by = null, locked_until = null
          WHERE id = ${j.id}
        `;
        await this._event(tx, {
          processInstanceId: instance.id,
          tokenId: row.id,
          jobId: j.id,
          eventType: 'job.cancelled',
          actor,
          data: { type: j.type, reason: 'branch skipped' },
        });
      }
    }

    const branchRows = await tx`SELECT id FROM tokens WHERE parent_token_id = ${parentId}`;
    const branchIds = branchRows.map((b: any) => b.id);

    const transition = node.transitions?.[0];
    if (!transition) {
      await this._checkProcessCompletion(tx, instance.id, actor);
      return;
    }

    const rows = await tx`
      INSERT INTO tokens (
        tenant_id, process_instance_id, parent_token_id, node_id, status, required
      ) VALUES (
        ${token.tenantId},
        ${token.processInstanceId},
        ${parentId},
        ${transition.to},
        'active',
        true
      )
      RETURNING *
    `;
    const newToken = this._mapToken(rows[0])!;

    await this._event(tx, {
      processInstanceId: instance.id,
      tokenId: newToken.id,
      eventType: 'token.joined',
      nodeId: node.id,
      actor,
      data: { joinNodeId: node.id, branches: branchIds, resultTokenId: newToken.id },
    });

    await this._arriveAtNode(tx, {
      token: newToken,
      instance,
      graph,
      actor,
      variables,
    });
  }

  private async _handleTimer(
    tx: SqlClient,
    token: Token,
    node: NodeDefinition,
    instance: ProcessInstance,
    actor: string,
    variables: Record<string, any>,
  ) {
    const timer = node.timer ?? {};
    let dueAt = this.now();
    if (timer.dueAt) {
      dueAt = new Date(timer.dueAt);
    } else if (timer.dueAtVariable) {
      const raw = variables[timer.dueAtVariable];
      if (raw) dueAt = new Date(raw);
    }

    const jobRows = await tx`
      INSERT INTO jobs (
        tenant_id, process_instance_id, token_id, type, due_at, payload, max_attempts
      ) VALUES (
        ${token.tenantId},
        ${instance.id},
        ${token.id},
        'timer',
        ${dueAt},
        ${JSON.stringify({ nodeId: node.id, transition: timer.transition ?? null })},
        5
      )
      RETURNING *
    `;
    const job = this._mapJob(jobRows[0]);

    await this._event(tx, {
      tenantId: token.tenantId,
      processInstanceId: instance.id,
      tokenId: token.id,
      jobId: job.id,
      eventType: 'timer.scheduled',
      nodeId: node.id,
      actor,
      data: { dueAt: dueAt.toISOString(), transition: timer.transition ?? null },
    });
    // Token remains active at the timer node until fired.
  }

  private async _handleCommand(
    tx: SqlClient,
    token: Token,
    node: NodeDefinition,
    instance: ProcessInstance,
    graph: ProcessGraph,
    actor: string,
    variables: Record<string, any>,
  ) {
    if (!this.app) {
      throw new MissingApplicationPortError(
        `Application port not configured for command node '${node.id}'`,
      );
    }
    const commandType = node.commandType;
    if (!commandType) {
      throw new Error(`Command node '${node.id}' has no commandType`);
    }

    let result: ApplicationCommandResult;
    let commandId: string;

    const existing = await tx`
      SELECT * FROM process_commands
      WHERE process_instance_id = ${instance.id} AND node_id = ${node.id}
    `;
    if (existing[0]) {
      // Idempotent replay: reuse the stored identity and outcome; never re-execute.
      commandId = existing[0].command_id;
      result = {
        commandId,
        outcome: existing[0].outcome,
        message: existing[0].message,
      };
      await this._event(tx, {
        processInstanceId: instance.id,
        tokenId: token.id,
        eventType: 'command.replayed',
        nodeId: node.id,
        actor,
        data: { commandId, outcome: result.outcome },
      });
    } else {
      commandId = this._commandId(instance.id, node.id);
      const input =
        node.inputMappings && Object.keys(node.inputMappings).length > 0
          ? node.inputMappings
          : variables;
      const request: ApplicationCommandRequest = {
        commandId,
        commandType,
        subjectType: instance.subjectType ?? null,
        subjectId: instance.subjectId ?? null,
        correlationId: instance.id,
        causationId: null,
        input,
      };

      await this._event(tx, {
        processInstanceId: instance.id,
        tokenId: token.id,
        eventType: 'command.requested',
        nodeId: node.id,
        actor,
        data: { commandId, commandType },
      });

      result = await this.app.executeCommand(request);

      await tx`
        INSERT INTO process_commands (
          process_instance_id, token_id, node_id, command_id, command_type,
          subject_type, subject_id, correlation_id, causation_id, input, outcome, message
        ) VALUES (
          ${instance.id},
          ${token.id},
          ${node.id},
          ${commandId},
          ${commandType},
          ${request.subjectType},
          ${request.subjectId},
          ${request.correlationId},
          ${request.causationId},
          ${JSON.stringify(input)},
          ${result.outcome},
          ${result.message ?? null}
        )
      `;
    }

    if (result.outcome === 'success') {
      await this._event(tx, {
        processInstanceId: instance.id,
        tokenId: token.id,
        eventType: 'command.completed',
        nodeId: node.id,
        actor,
        data: { commandId, commandType },
      });

      let currentVariables = variables;
      if (this.app) {
        currentVariables = await this._refreshFacts(tx, { ...instance, variables }, variables);
      }

      const transitionName = node.transition ?? node.transitions?.[0]?.name;
      const transition =
        node.transitions?.find((t) => t.name === transitionName) ?? node.transitions?.[0];
      if (!transition) {
        throw new Error(`Command node ${node.id} has no success transition`);
      }

      await this._moveToken(tx, token, transition.to, transition.name, actor);
      const fresh = this._mapToken(
        (await tx`SELECT * FROM tokens WHERE id = ${token.id}`)[0],
      );
      await this._arriveAtNode(tx, {
        token: fresh!,
        instance: { ...instance, variables: currentVariables },
        graph,
        actor,
        variables: currentVariables,
      });
      return;
    }

    if (result.outcome === 'conflict') {
      await this._event(tx, {
        processInstanceId: instance.id,
        tokenId: token.id,
        eventType: 'command.failed',
        nodeId: node.id,
        actor,
        data: { commandId, outcome: 'conflict', message: result.message },
      });
      await this._terminateProcess(
        tx,
        instance.id,
        actor,
        'conflict',
        result.message ?? 'command conflict',
      );
      return;
    }

    // validation_failure | not_found | unauthorized | precondition_failure
    await this._event(tx, {
      processInstanceId: instance.id,
      tokenId: token.id,
      eventType: 'command.failed',
      nodeId: node.id,
      actor,
      data: { commandId, outcome: result.outcome, message: result.message },
    });
    await this._terminateProcess(
      tx,
      instance.id,
      actor,
      'failed',
      result.message ?? `command outcome: ${result.outcome}`,
    );
  }

  private async _refreshFacts(
    tx: SqlClient,
    instance: ProcessInstance,
    variables: Record<string, any>,
  ): Promise<Record<string, any>> {
    if (!this.app || !instance.subjectType || !instance.subjectId) return variables;
    const facts = await this.app.readFacts({
      subjectType: instance.subjectType,
      subjectId: instance.subjectId,
    });
    const merged = { ...variables, ...facts };
    await tx`
      UPDATE process_instances
      SET variables = ${JSON.stringify(merged)}, version = version + 1
      WHERE id = ${instance.id}
    `;
    return merged;
  }

  private _commandId(processInstanceId: string, nodeId: string): string {
    return createHash('sha256').update(`${processInstanceId}:${nodeId}`).digest('hex');
  }

  private async _event(
    tx: SqlClient,
    opts: {
      tenantId?: string | null;
      processInstanceId: string;
      tokenId?: string | null;
      taskId?: string | null;
      jobId?: string | null;
      eventType: string;
      nodeId?: string | null;
      actor: string;
      data?: Record<string, any>;
    },
  ) {
    await tx`
      INSERT INTO process_events (
        tenant_id, process_instance_id, token_id, task_id, job_id, event_type, node_id, actor, data
      ) VALUES (
        ${opts.tenantId ?? null},
        ${opts.processInstanceId},
        ${opts.tokenId ?? null},
        ${opts.taskId ?? null},
        ${opts.jobId ?? null},
        ${opts.eventType},
        ${opts.nodeId ?? null},
        ${opts.actor},
        ${JSON.stringify(opts.data ?? {})}
      )
    `;
  }

  private async _loadDefinition(
    tx: SqlClient,
    key: string,
    version?: number,
    tenantId?: string,
  ): Promise<ProcessDefinition> {
    const rows = version
      ? await tx`
          SELECT * FROM process_definitions
          WHERE key = ${key} AND version = ${version} AND status = 'active'
            ${tenantId ? tx`AND tenant_id = ${tenantId}` : tx`AND tenant_id IS NULL`}
        `
      : await tx`
          SELECT * FROM process_definitions
          WHERE key = ${key} AND status = 'active'
            ${tenantId ? tx`AND tenant_id = ${tenantId}` : tx`AND tenant_id IS NULL`}
          ORDER BY version DESC
          LIMIT 1
        `;

    if (!rows[0]) {
      throw new Error(`Process definition not found: ${key}${version ? ` v${version}` : ''}`);
    }
    return this._mapDefinition(rows[0]);
  }

  // ----------------------------------------------------------------
  // Row mappers (snake_case → camelCase)
  // ----------------------------------------------------------------
  private _mapInstance(row: any): ProcessInstance {
    return {
      id: row.id,
      tenantId: row.tenant_id,
      definitionId: row.definition_id,
      businessKey: row.business_key,
      status: row.status,
      outcome: row.outcome ?? null,
      startedAt: row.started_at,
      endedAt: row.ended_at,
      startedBy: row.started_by,
      parentInstanceId: row.parent_instance_id,
      rootTokenId: row.root_token_id,
      subjectType: row.subject_type ?? null,
      subjectId: row.subject_id ?? null,
      variables: row.variables ?? {},
      version: row.version,
      createdAt: row.created_at,
      updatedAt: row.updated_at,
    };
  }

  private _mapToken(row: any): Token | null {
    if (!row) return null;
    return {
      id: row.id,
      tenantId: row.tenant_id,
      processInstanceId: row.process_instance_id,
      parentTokenId: row.parent_token_id,
      nodeId: row.node_id,
      status: row.status,
      outcome: row.outcome ?? null,
      required: row.required ?? true,
      isAbleToReactivateParent: row.is_able_to_reactivate_parent,
      startedAt: row.started_at,
      endedAt: row.ended_at,
      version: row.version,
      createdAt: row.created_at,
      updatedAt: row.updated_at,
    };
  }

  private _mapTask(row: any): Task | null {
    if (!row) return null;
    return {
      id: row.id,
      tenantId: row.tenant_id,
      processInstanceId: row.process_instance_id,
      tokenId: row.token_id,
      name: row.name,
      description: row.description,
      status: row.status,
      assignee: row.assignee,
      candidates: row.candidates ?? [],
      swimlane: row.swimlane,
      priority: row.priority,
      dueDate: row.due_date,
      formKey: row.form_key,
      formData: row.form_data ?? {},
      createdAt: row.created_at,
      claimedAt: row.claimed_at,
      completedAt: row.completed_at,
      completedBy: row.completed_by,
      version: row.version,
      updatedAt: row.updated_at,
    };
  }

  private _mapJob(row: any): Job {
    return {
      id: row.id,
      tenantId: row.tenant_id,
      processInstanceId: row.process_instance_id,
      tokenId: row.token_id,
      type: row.type,
      dueAt: row.due_at,
      status: row.status,
      lockedBy: row.locked_by,
      lockedUntil: row.locked_until,
      attempts: row.attempts,
      maxAttempts: row.max_attempts,
      payload: row.payload ?? {},
      lastError: row.last_error,
      createdAt: row.created_at,
      updatedAt: row.updated_at,
      completedAt: row.completed_at,
    };
  }

  private _mapDefinition(row: any): ProcessDefinition {
    return {
      id: row.id,
      tenantId: row.tenant_id,
      key: row.key,
      version: row.version,
      name: row.name,
      description: row.description,
      definition: row.definition,
      status: row.status,
      createdAt: row.created_at,
      createdBy: row.created_by,
      updatedAt: row.updated_at,
    };
  }
}
