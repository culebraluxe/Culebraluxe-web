import { ProcessInstance, Token, Task } from '@/lib/workflow/types';
import Link from 'next/link';

interface ProcessDetailProps {
  details: ProcessInstance & {
    definition: any;
    tokens: Token[];
    tasks: Task[];
  };
  history: any[];
}

export function ProcessDetail({ details, history }: ProcessDetailProps) {
  const { tokens, tasks, definition } = details;

  return (
    <div className="space-y-8">
      <div className="flex justify-between items-start">
        <div>
          <h1 className="text-2xl font-bold">
            {definition?.name ?? 'Process'} — {details.id.slice(0, 8)}
          </h1>
          <p className="text-gray-600 mt-1">
            Status:{' '}
            <span className="capitalize font-medium">{details.status}</span>
            {details.businessKey && (
              <> · Business Key: {details.businessKey}</>
            )}
          </p>
        </div>
        <Link href="/dashboard" className="text-sm text-blue-600 hover:underline">
          ← Back to Dashboard
        </Link>
      </div>

      <section className="border rounded-lg p-5 bg-white shadow-sm">
        <h2 className="font-semibold mb-3">Variables</h2>
        <pre className="text-sm bg-gray-50 p-3 rounded overflow-auto">
          {JSON.stringify(details.variables, null, 2)}
        </pre>
      </section>

      <section className="border rounded-lg p-5 bg-white shadow-sm">
        <h2 className="font-semibold mb-3">Tokens</h2>
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b text-left">
                <th className="py-2 pr-4">ID</th>
                <th className="py-2 pr-4">Node</th>
                <th className="py-2 pr-4">Status</th>
                <th className="py-2 pr-4">Parent</th>
                <th className="py-2">Started</th>
              </tr>
            </thead>
            <tbody>
              {tokens.map((t) => (
                <tr key={t.id} className="border-b last:border-0">
                  <td className="py-2 pr-4 font-mono text-xs">{t.id.slice(0, 8)}</td>
                  <td className="py-2 pr-4">{t.nodeId}</td>
                  <td className="py-2 pr-4 capitalize">{t.status}</td>
                  <td className="py-2 pr-4 font-mono text-xs">
                    {t.parentTokenId?.slice(0, 8) ?? '—'}
                  </td>
                  <td className="py-2">{new Date(t.startedAt).toLocaleString()}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>

      <section className="border rounded-lg p-5 bg-white shadow-sm">
        <h2 className="font-semibold mb-3">Tasks</h2>
        {tasks.length === 0 ? (
          <p className="text-gray-500 text-sm">No tasks</p>
        ) : (
          <div className="space-y-2">
            {tasks.map((task) => (
              <Link
                key={task.id}
                href={`/tasks/${task.id}`}
                className="block p-3 border rounded hover:bg-gray-50"
              >
                <div className="flex justify-between">
                  <span className="font-medium">{task.name}</span>
                  <span className="text-sm capitalize text-gray-600">{task.status}</span>
                </div>
                {task.assignee && (
                  <p className="text-sm text-gray-500 mt-1">Assignee: {task.assignee}</p>
                )}
              </Link>
            ))}
          </div>
        )}
      </section>

      <section className="border rounded-lg p-5 bg-white shadow-sm">
        <h2 className="font-semibold mb-4">History</h2>
        <div className="space-y-4">
          {history.map((event) => (
            <div key={event.id} className="flex gap-4">
              <div className="w-32 text-xs text-gray-500 pt-1 shrink-0">
                {new Date(event.created_at).toLocaleString()}
              </div>
              <div className="flex-1">
                <div className="font-medium text-sm">{event.event_type}</div>
                <div className="text-sm text-gray-600">
                  {event.node_id && <span>Node: {event.node_id} · </span>}
                  {event.actor && <span>Actor: {event.actor}</span>}
                </div>
                {event.data && Object.keys(event.data).length > 0 && (
                  <pre className="mt-1 text-xs bg-gray-50 p-2 rounded overflow-auto">
                    {JSON.stringify(event.data, null, 2)}
                  </pre>
                )}
              </div>
            </div>
          ))}
        </div>
      </section>
    </div>
  );
}
