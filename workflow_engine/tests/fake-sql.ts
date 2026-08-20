// Deterministic in-memory SQL fake for the WorkflowEngine. It implements the
// neon tagged-template surface the engine actually uses (`sql`, `sql.begin`,
// nested fragments) against simple in-memory tables. Not production code.

type Row = Record<string, any>;

export interface Store {
  processDefinitions: Row[];
  processInstances: Row[];
  tokens: Row[];
  tasks: Row[];
  jobs: Row[];
  processEvents: Row[];
  processCommands: Row[];
}

type Fragment = { strings: readonly string[]; values: any[] };

function isFragment(v: any): v is Fragment {
  return (
    v != null &&
    typeof v === 'object' &&
    Array.isArray((v as Fragment).strings) &&
    Array.isArray((v as Fragment).values)
  );
}

function flatten(strings: readonly string[], values: any[]) {
  let text = '';
  const params: any[] = [];
  const walk = (s: readonly string[], v: any[]) => {
    for (let i = 0; i < s.length; i++) {
      text += s[i];
      if (i < v.length) {
        const val = v[i];
        if (isFragment(val)) {
          walk(val.strings, val.values);
        } else {
          params.push(val);
          text += '$' + params.length;
        }
      }
    }
  };
  walk(strings, values);
  return { text, params };
}

const norm = (text: string) => text.replace(/\s+/g, ' ').trim().toLowerCase();

let uuidSeq = 0;
const genUuid = () => `uuid-${++uuidSeq}`;

export class FakeSql {
  store: Store = {
    processDefinitions: [],
    processInstances: [],
    tokens: [],
    tasks: [],
    jobs: [],
    processEvents: [],
    processCommands: [],
  };

  private eventSeq = 0;
  private commandSeq = 0;

  readonly sql: any;

  constructor() {
    const self = this;
    this.sql = Object.assign(
      (strings: readonly string[], ...values: any[]) =>
        self._makeQuery(strings, values),
      {
        begin: (cb: (tx: any) => Promise<any>) => self._begin(cb),
      },
    );
  }

  private _makeQuery(strings: readonly string[], values: any[]) {
    const self = this;
    return {
      strings,
      values,
      then(resolve: any, reject: any) {
        try {
          return Promise.resolve(self._run(strings, values)).then(resolve, reject);
        } catch (err) {
          return Promise.reject(err).then(resolve, reject);
        }
      },
    };
  }

  private async _begin(cb: (tx: any) => Promise<any>) {
    const snapshot = structuredClone(this.store);
    const self = this;
    const tx = (strings: readonly string[], ...values: any[]) =>
      self._makeQuery(strings, values);
    try {
      return await cb(tx);
    } catch (err) {
      this.store = snapshot; // rollback
      throw err;
    }
  }

  private _run(strings: readonly string[], values: any[]) {
    const { text, params } = flatten(strings, values);
    const t = norm(text);
    const now = new Date();
    const st = this.store;

    // ------------------------------------------------------------------
    // SELECT
    // ------------------------------------------------------------------
    if (t.startsWith('select count(*)::int as cnt from tokens where process_instance_id =') && t.includes("status = 'active'") && !t.includes('parent_token_id')) {
      const pid = params[0];
      const n = st.tokens.filter((r) => r.process_instance_id === pid && r.status === 'active').length;
      return [{ cnt: n }];
    }
    if (t.startsWith('select count(*)::int as cnt from tokens where parent_token_id =') && t.includes("status = 'active' and required = true")) {
      const parent = params[0];
      const n = st.tokens.filter((r) => r.parent_token_id === parent && r.status === 'active' && r.required === true).length;
      return [{ cnt: n }];
    }
    if (t.startsWith('select * from tokens where parent_token_id =') && t.includes("status = 'active' and required = false")) {
      const parent = params[0];
      return st.tokens.filter((r) => r.parent_token_id === parent && r.status === 'active' && r.required === false);
    }
    if (t.startsWith('select id from tokens where parent_token_id =')) {
      const parent = params[0];
      return st.tokens.filter((r) => r.parent_token_id === parent).map((r) => ({ id: r.id }));
    }
    if (t.startsWith('select * from tokens where process_instance_id =') && t.includes("and status = 'active'")) {
      const pid = params[0];
      return st.tokens.filter((r) => r.process_instance_id === pid && r.status === 'active');
    }
    if (t.startsWith('select * from tokens where process_instance_id =') && t.includes('order by created_at')) {
      const pid = params[0];
      return st.tokens.filter((r) => r.process_instance_id === pid);
    }
    if (t.startsWith('select * from tokens where id =') && t.includes('for update')) {
      const row = st.tokens.find((r) => r.id === params[0]);
      return row ? [row] : [];
    }
    if (t.startsWith('select * from tokens where id =')) {
      const row = st.tokens.find((r) => r.id === params[0]);
      return row ? [row] : [];
    }

    if (t.startsWith('select * from process_instances where id =') && t.includes('for update')) {
      const row = st.processInstances.find((r) => r.id === params[0]);
      return row ? [row] : [];
    }
    if (t.startsWith('select * from process_instances where id =')) {
      const row = st.processInstances.find((r) => r.id === params[0]);
      return row ? [row] : [];
    }

    if (t.startsWith('select id, key, version, name, status from process_definitions where id =')) {
      const row = st.processDefinitions.find((r) => r.id === params[0]);
      if (!row) return [];
      return [{ id: row.id, key: row.key, version: row.version, name: row.name, status: row.status }];
    }
    if (t.startsWith('select * from process_definitions where key =') && t.includes('and version =')) {
      const key = params[0];
      const version = params[1];
      const tenantId = t.includes('tenant_id = $3') ? params[2] : null;
      return st.processDefinitions.filter(
        (r) => r.key === key && r.version === version && r.status === 'active' && (tenantId ? r.tenant_id === tenantId : r.tenant_id == null),
      );
    }
    if (t.startsWith('select * from process_definitions where key =') && t.includes('order by version desc')) {
      const key = params[0];
      const tenantId = t.includes('tenant_id = $2') ? params[1] : null;
      return st.processDefinitions
        .filter((r) => r.key === key && r.status === 'active' && (tenantId ? r.tenant_id === tenantId : r.tenant_id == null))
        .sort((a, b) => b.version - a.version)
        .slice(0, 1);
    }
    if (t.startsWith('select * from process_definitions where id =')) {
      const row = st.processDefinitions.find((r) => r.id === params[0]);
      return row ? [row] : [];
    }

    if (t.startsWith('select id from tasks where process_instance_id =') && t.includes("status in ('ready', 'reserved', 'in_progress')")) {
      const pid = params[0];
      return st.tasks.filter((r) => r.process_instance_id === pid && ['ready', 'reserved', 'in_progress'].includes(r.status)).map((r) => ({ id: r.id }));
    }
    if (t.startsWith('select id from tasks where token_id =') && t.includes("status in ('ready', 'reserved', 'in_progress')")) {
      const tid = params[0];
      return st.tasks.filter((r) => r.token_id === tid && ['ready', 'reserved', 'in_progress'].includes(r.status)).map((r) => ({ id: r.id }));
    }
    if (t.startsWith('select * from tasks where process_instance_id =') && t.includes('order by created_at')) {
      const pid = params[0];
      return st.tasks.filter((r) => r.process_instance_id === pid);
    }
    if (t.startsWith('select * from tasks where status in') && t.includes('= any(candidates)')) {
      const userId = params[0];
      const tenantId = t.includes('tenant_id = $3') ? params[2] : undefined;
      return st.tasks
        .filter((r) => ['ready', 'reserved', 'in_progress'].includes(r.status))
        .filter((r) => r.assignee === userId || (Array.isArray(r.candidates) && r.candidates.includes(userId)))
        .filter((r) => (tenantId === undefined ? true : r.tenant_id === tenantId))
        .sort((a, b) => (b.priority ?? 0) - (a.priority ?? 0));
    }
    if (t.startsWith('select * from tasks where id =') && t.includes('for update')) {
      const row = st.tasks.find((r) => r.id === params[0]);
      return row ? [row] : [];
    }
    if (t.startsWith('select * from tasks where id =')) {
      const row = st.tasks.find((r) => r.id === params[0]);
      return row ? [row] : [];
    }

    if (t.startsWith('select id from jobs where process_instance_id =') && t.includes("status in ('pending', 'locked')")) {
      const pid = params[0];
      return st.jobs.filter((r) => r.process_instance_id === pid && ['pending', 'locked'].includes(r.status)).map((r) => ({ id: r.id }));
    }
    if (t.startsWith('select * from jobs where id =') && t.includes('for update')) {
      const row = st.jobs.find((r) => r.id === params[0]);
      return row ? [row] : [];
    }

    if (t.startsWith('select * from process_commands where process_instance_id =') && t.includes('and node_id =')) {
      return st.processCommands.filter((r) => r.process_instance_id === params[0] && r.node_id === params[1]);
    }

    if (t.startsWith('select * from process_events where process_instance_id =') && t.includes('order by created_at desc')) {
      const pid = params[0];
      const limit = params[1];
      return st.processEvents
        .filter((r) => r.process_instance_id === pid)
        .sort((a, b) => (a.id < b.id ? 1 : a.id > b.id ? -1 : 0))
        .slice(0, limit);
    }

    if (t.startsWith('select pi.* from process_instances pi')) {
      let idx = 0;
      let tenantId: any, status: any, definitionKey: any, businessKey: any;
      if (t.includes('pi.tenant_id = $')) tenantId = params[idx++];
      if (t.includes('pi.status = any($')) status = params[idx++];
      else if (t.includes('pi.status = $')) status = params[idx++];
      if (t.includes('pd.key = $')) definitionKey = params[idx++];
      if (t.includes('pi.business_key = $')) businessKey = params[idx++];
      const limit = params[idx];
      const offset = params[idx + 1];

      let rows = st.processInstances.filter((r) => {
        if (tenantId !== undefined && r.tenant_id !== tenantId) return false;
        if (status !== undefined) {
          if (Array.isArray(status) ? !status.includes(r.status) : r.status !== status) return false;
        }
        if (definitionKey !== undefined) {
          const def = st.processDefinitions.find((d) => d.id === r.definition_id);
          if (!def || def.key !== definitionKey) return false;
        }
        if (businessKey !== undefined && r.business_key !== businessKey) return false;
        return true;
      });
      rows = rows.sort((a, b) => (a.started_at < b.started_at ? 1 : a.started_at > b.started_at ? -1 : 0));
      return rows.slice(offset ?? 0, (offset ?? 0) + (limit ?? 50));
    }

    // ------------------------------------------------------------------
    // INSERT
    // ------------------------------------------------------------------
    if (t.startsWith('insert into process_instances (')) {
      const row: Row = {
        id: genUuid(),
        tenant_id: params[0],
        definition_id: params[1],
        business_key: params[2],
        status: 'active',
        started_at: now,
        ended_at: null,
        started_by: params[3],
        parent_instance_id: null,
        root_token_id: null,
        subject_type: params[5],
        subject_id: params[6],
        variables: JSON.parse(params[4]),
        outcome: null,
        version: 1,
        created_at: now,
        updated_at: now,
      };
      st.processInstances.push(row);
      return [{ id: row.id }];
    }

    if (t.startsWith('insert into tokens (')) {
      const hasParent = t.includes('parent_token_id');
      const row: Row = {
        id: genUuid(),
        tenant_id: params[0],
        process_instance_id: params[1],
        parent_token_id: hasParent ? params[2] : null,
        node_id: hasParent ? params[3] : params[2],
        status: 'active',
        required: hasParent ? params[4] : true,
        outcome: null,
        is_able_to_reactivate_parent: true,
        started_at: now,
        ended_at: null,
        version: 1,
        created_at: now,
        updated_at: now,
      };
      st.tokens.push(row);
      return t.includes('returning *') ? [{ ...row }] : [{ id: row.id }];
    }

    if (t.startsWith('insert into tasks (')) {
      const row: Row = {
        id: genUuid(),
        tenant_id: params[0],
        process_instance_id: params[1],
        token_id: params[2],
        name: params[3],
        description: params[4],
        status: 'ready',
        assignee: null,
        candidates: params[5],
        swimlane: null,
        priority: params[7],
        due_date: null,
        form_key: params[6],
        form_data: {},
        created_at: now,
        claimed_at: null,
        completed_at: null,
        completed_by: null,
        version: 1,
        updated_at: now,
      };
      st.tasks.push(row);
      return [{ id: row.id }];
    }

    if (t.startsWith('insert into jobs (')) {
      const isTimer = t.includes("'timer'");
      const row: Row = {
        id: genUuid(),
        tenant_id: params[0],
        process_instance_id: params[1],
        token_id: params[2],
        type: isTimer ? 'timer' : params[3],
        due_at: isTimer ? params[3] : params[4],
        status: 'pending',
        locked_by: null,
        locked_until: null,
        attempts: 0,
        max_attempts: isTimer ? 5 : params[6],
        payload: JSON.parse(isTimer ? params[4] : params[5]),
        last_error: null,
        created_at: now,
        updated_at: now,
        completed_at: null,
      };
      st.jobs.push(row);
      return t.includes('returning *') ? [{ ...row }] : [{ id: row.id }];
    }

    if (t.startsWith('insert into process_events (')) {
      const row: Row = {
        id: ++this.eventSeq,
        tenant_id: params[0],
        process_instance_id: params[1],
        token_id: params[2],
        task_id: params[3],
        job_id: params[4],
        event_type: params[5],
        node_id: params[6],
        actor: params[7],
        data: JSON.parse(params[8]),
        created_at: now,
      };
      st.processEvents.push(row);
      return [];
    }

    if (t.startsWith('insert into process_commands (')) {
      const row: Row = {
        id: ++this.commandSeq,
        process_instance_id: params[0],
        token_id: params[1],
        node_id: params[2],
        command_id: params[3],
        command_type: params[4],
        subject_type: params[5],
        subject_id: params[6],
        correlation_id: params[7],
        causation_id: params[8],
        input: JSON.parse(params[9]),
        outcome: params[10],
        message: params[11],
        created_at: now,
      };
      st.processCommands.push(row);
      return [];
    }

    // ------------------------------------------------------------------
    // UPDATE
    // ------------------------------------------------------------------
    if (t.startsWith('update process_instances set root_token_id =')) {
      const row = st.processInstances.find((r) => r.id === params[1]);
      if (row) row.root_token_id = params[0];
      return [];
    }
    if (t.startsWith('update process_instances set variables =') && t.includes('version = version + 1')) {
      const row = st.processInstances.find((r) => r.id === params[1]);
      if (row) {
        row.variables = JSON.parse(params[0]);
        row.version += 1;
      }
      return [];
    }
    if (t.startsWith('update process_instances set status =') && t.includes("outcome = 'completed'")) {
      const row = st.processInstances.find((r) => r.id === params[1]);
      if (row) {
        row.status = 'completed';
        row.outcome = 'completed';
        row.ended_at = params[0];
        row.version += 1;
      }
      return [];
    }
    if (t.startsWith('update process_instances set status =') && t.includes('outcome =')) {
      const row = st.processInstances.find((r) => r.id === params[3]);
      if (row) {
        row.status = params[0];
        row.outcome = params[1];
        row.ended_at = params[2];
        row.version += 1;
      }
      return [];
    }

    if (t.startsWith('update tokens set node_id =') && t.includes('returning id')) {
      const row = st.tokens.find((r) => r.id === params[1] && r.version === params[2]);
      if (!row) return [];
      row.node_id = params[0];
      row.version += 1;
      return [{ id: row.id }];
    }
    if (t.startsWith('update tokens set status =') && t.includes("outcome = 'cancelled'")) {
      const row = st.tokens.find((r) => r.id === params[1]);
      if (row) {
        row.status = 'completed';
        row.ended_at = params[0];
        row.outcome = 'cancelled';
        row.version += 1;
      }
      return [];
    }
    if (t.startsWith('update tokens set status =') && t.includes("outcome = 'skipped'")) {
      const row = st.tokens.find((r) => r.id === params[1]);
      if (row) {
        row.status = 'completed';
        row.ended_at = params[0];
        row.outcome = 'skipped';
        row.version += 1;
      }
      return [];
    }
    if (t.startsWith('update tokens set status =') && t.includes('outcome =')) {
      // _completeToken (generic outcome)
      const row = st.tokens.find((r) => r.id === params[2]);
      if (row) {
        row.status = 'completed';
        row.ended_at = params[0];
        row.outcome = params[1];
        row.version += 1;
      }
      return [];
    }

    if (t.startsWith('update tasks set status =') && t.includes("'reserved'")) {
      const row = st.tasks.find((r) => r.id === params[2] && r.version === params[3]);
      if (!row) return [];
      row.status = 'reserved';
      row.assignee = params[0];
      row.claimed_at = params[1];
      row.version += 1;
      return [];
    }
    if (t.startsWith('update tasks set status =') && t.includes("'ready'")) {
      const row = st.tasks.find((r) => r.id === params[0] && r.version === params[1]);
      if (!row) return [];
      row.status = 'ready';
      row.assignee = null;
      row.claimed_at = null;
      row.version += 1;
      return [];
    }
    if (t.startsWith('update tasks set status =') && t.includes("'completed'") && t.includes('form_data =')) {
      const row = st.tasks.find((r) => r.id === params[4] && r.version === params[5]);
      if (!row) return [];
      row.status = 'completed';
      row.assignee = params[0];
      row.form_data = JSON.parse(params[1]);
      row.completed_at = params[2];
      row.completed_by = params[3];
      row.version += 1;
      return [];
    }
    if (t.startsWith('update tasks set status =') && t.includes("'obsolete'")) {
      const row = st.tasks.find((r) => r.id === params[0]);
      if (row) {
        row.status = 'obsolete';
        row.version += 1;
      }
      return [];
    }

    if (t.startsWith('update jobs set status =') && t.includes("'locked'") && t.includes('skip locked')) {
      const nowParam = params[2];
      const dueParam = params[3];
      const limit = params[4];
      const eligible = st.jobs
        .filter((r) => r.status === 'pending' && r.due_at <= dueParam && r.attempts < r.max_attempts)
        .sort((a, b) => (a.due_at < b.due_at ? -1 : a.due_at > b.due_at ? 1 : 0))
        .slice(0, limit);
      const out: Row[] = [];
      for (const r of eligible) {
        r.status = 'locked';
        r.locked_by = params[0];
        r.locked_until = params[1];
        r.attempts += 1;
        r.updated_at = nowParam;
        out.push({ ...r });
      }
      return out;
    }
    if (t.startsWith('update jobs set status =') && t.includes("'completed'") && t.includes('completed_at =')) {
      const row = st.jobs.find((r) => r.id === params[1]);
      if (row) {
        row.status = 'completed';
        row.completed_at = params[0];
        row.locked_by = null;
        row.locked_until = null;
      }
      return [];
    }
    if (t.startsWith('update jobs set status =') && t.includes('last_error =')) {
      const row = st.jobs.find((r) => r.id === params[3]);
      if (row) {
        row.status = params[0];
        row.last_error = params[1];
        row.locked_by = null;
        row.locked_until = null;
        row.due_at = params[2];
      }
      return [];
    }
    if (t.startsWith('update jobs set status =') && t.includes("'cancelled'") && t.includes('locked_by = null')) {
      const row = st.jobs.find((r) => r.id === params[0]);
      if (row) {
        row.status = 'cancelled';
        row.locked_by = null;
        row.locked_until = null;
      }
      return [];
    }
    if (t.startsWith('update jobs set status =') && t.includes("'cancelled'") && !t.includes('locked_by')) {
      const row = st.jobs.find((r) => r.id === params[0]);
      if (row) row.status = 'cancelled';
      return [];
    }
    if (t.startsWith('update jobs set due_at =') && t.includes('updated_at =')) {
      const row = st.jobs.find((r) => r.id === params[2]);
      if (row) {
        row.due_at = params[0];
        row.updated_at = params[1];
      }
      return [];
    }

    throw new Error(`FAKE_SQL_UNHANDLED: ${t}`);
  }

  // ------------------------------------------------------------------
  // Test helpers
  // ------------------------------------------------------------------
  seedDefinition(key: string, version: number, graph: any, opts: { tenantId?: string | null } = {}) {
    const now = new Date();
    const row: Row = {
      id: genUuid(),
      tenant_id: opts.tenantId ?? null,
      key,
      version,
      name: key,
      description: null,
      definition: graph,
      status: 'active',
      created_at: now,
      created_by: null,
      updated_at: now,
    };
    this.store.processDefinitions.push(row);
    return row.id;
  }
}
