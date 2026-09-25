/** Runtime-only architecture fence.
 *
 * The roots are discovered from Next App Router entrypoints by
 * scripts/app-runtime-boundary.mjs. Dead migration carcass is intentionally not
 * a root: it can remain until the final deletion pass, but the executable app
 * may not reach it.
 */
module.exports = {
  forbidden: [
    {
      name: 'no-runtime-legacy',
      severity: 'error',
      comment: 'Executable app code must use Rust services, never legacy TypeScript.',
      from: {},
      to: { path: '^legacy/' },
    },
    {
      name: 'no-runtime-workflow-engine',
      severity: 'error',
      comment: 'Executable app code must not fall back into the TypeScript workflow engine.',
      from: {},
      to: { path: '^workflow_engine/' },
    },
    {
      name: 'no-runtime-agent-runtime',
      severity: 'error',
      comment: 'Executable app code must not fall back into the TypeScript agent runtime.',
      from: {},
      to: { path: '^agent-runtime/' },
    },
  ],
  options: {
    doNotFollow: { path: 'node_modules' },
    tsConfig: { fileName: 'tsconfig.json' },
    tsPreCompilationDeps: false,
    exclude: { path: '(\\.next|node_modules|testv2|coverage)' },
  },
}
