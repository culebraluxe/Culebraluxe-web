/** @type {import('dependency-cruiser').IConfiguration} */
module.exports = {
  forbidden: [
    {
      name: 'no-circular',
      severity: 'info',
      comment: 'Architecture signal: report circular dependencies (fix incrementally; surfaced for review, not a hard fail yet).',
      from: {},
      to: { circular: true },
    },
    {
      name: 'no-app-imports-workflow-internals',
      severity: 'warn',
      comment: 'App/UI layers must not reach into workflow_engine internals; use the exported runtime.',
      from: { path: '^(app|components|ui)/' },
      to: { path: '^workflow_engine/' },
    },
    {
      name: 'no-db-import-from-components',
      severity: 'error',
      comment: 'Components must not touch the db layer directly; go through a service/lib seam.',
      from: { path: '^components/' },
      to: { path: '^(db|neon\\.ts)/' },
    },
  ],
  options: {
    doNotFollow: { path: 'node_modules' },
    tsConfig: { fileName: 'tsconfig.json' },
    tsPreCompilationDeps: false,
    exclude: { path: '(\\.next|node_modules|testv2|coverage)' },
  },
}
