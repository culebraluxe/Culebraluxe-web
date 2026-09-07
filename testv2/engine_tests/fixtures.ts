import type {
  ApplicationPort,
  ApplicationCommandResult,
  ApplicationFacts,
} from '../../workflow_engine/lib/workflow/types';

// Minimal deterministic condition evaluator for tests (the engine default is
// expr-eval; tests inject this stub so no package is required).
export function stubEvaluator(
  expression: string,
  variables: Record<string, any>,
): boolean {
  const t = expression.trim();
  if (t === 'true') return true;
  if (t === 'false') return false;
  const m = t.match(
    /^([A-Za-z_][A-Za-z0-9_]*)\s*(===|==|!==|!=|>=|<=|>|<)\s*(.+)$/,
  );
  if (!m) return false;
  const [, name, op, rhsRaw] = m;
  const lhs = variables[name];
  const rhs = parseValue(rhsRaw.trim());
  switch (op) {
    case '>':
      return Number(lhs) > Number(rhs);
    case '<':
      return Number(lhs) < Number(rhs);
    case '>=':
      return Number(lhs) >= Number(rhs);
    case '<=':
      return Number(lhs) <= Number(rhs);
    case '==':
    case '===':
      return lhs === rhs;
    case '!=':
    case '!==':
      return lhs !== rhs;
    default:
      return false;
  }
}

function parseValue(s: string): any {
  if (/^['"].*['"]$/.test(s)) return s.slice(1, -1);
  if (s === 'true') return true;
  if (s === 'false') return false;
  if (s === 'null') return null;
  if (s === 'undefined') return undefined;
  const n = Number(s);
  return Number.isNaN(n) ? s : n;
}

export interface AppOverrides {
  executeCommand?: (req: any) => Promise<ApplicationCommandResult>;
  readFacts?: (subject: {
    subjectType: string;
    subjectId: string;
  }) => Promise<ApplicationFacts>;
}

export type TestApp = ApplicationPort & {
  calls: any[];
  factCalls: any[];
};

export function makeApp(overrides: AppOverrides = {}): TestApp {
  const calls: any[] = [];
  const factCalls: any[] = [];
  return {
    calls,
    factCalls,
    async executeCommand(req: any): Promise<ApplicationCommandResult> {
      calls.push(req);
      if (overrides.executeCommand) return overrides.executeCommand(req);
      return { commandId: req.commandId, outcome: 'success' };
    },
    async readFacts(subject: any): Promise<ApplicationFacts> {
      factCalls.push(subject);
      if (overrides.readFacts) return overrides.readFacts(subject);
      return {};
    },
  };
}
