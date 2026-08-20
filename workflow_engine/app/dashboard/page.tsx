import { engine } from '@/lib/workflow/db';
import Link from 'next/link';
import { TaskList } from '@/components/workflow/TaskList';

export default async function DashboardPage() {
  const currentUserId = 'john.doe';

  const [myTasks, recentInstances] = await Promise.all([
    engine.getActiveTasksForUser(currentUserId),
    engine.findProcessInstances({ limit: 10 }),
  ]);

  return (
    <div className="max-w-6xl mx-auto py-10 px-4 space-y-10">
      <div className="flex justify-between items-center">
        <h1 className="text-2xl font-bold">Workflow Dashboard</h1>
        <Link
          href="/processes/start"
          className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700"
        >
          Start New Process
        </Link>
      </div>

      <section>
        <h2 className="text-lg font-semibold mb-4">My Active Tasks</h2>
        <TaskList tasks={myTasks} currentUserId={currentUserId} />
      </section>

      <section>
        <h2 className="text-lg font-semibold mb-4">Recent Processes</h2>
        <div className="border rounded-lg overflow-hidden bg-white shadow-sm">
          <table className="w-full text-sm">
            <thead className="bg-gray-50 text-left">
              <tr>
                <th className="px-4 py-3">ID</th>
                <th className="px-4 py-3">Business Key</th>
                <th className="px-4 py-3">Status</th>
                <th className="px-4 py-3">Started</th>
                <th className="px-4 py-3">Started By</th>
              </tr>
            </thead>
            <tbody>
              {recentInstances.map((pi) => (
                <tr key={pi.id} className="border-t hover:bg-gray-50">
                  <td className="px-4 py-3">
                    <Link
                      href={`/processes/${pi.id}`}
                      className="text-blue-600 hover:underline font-mono text-xs"
                    >
                      {pi.id.slice(0, 8)}…
                    </Link>
                  </td>
                  <td className="px-4 py-3">{pi.businessKey ?? '—'}</td>
                  <td className="px-4 py-3 capitalize">{pi.status}</td>
                  <td className="px-4 py-3">
                    {new Date(pi.startedAt).toLocaleString()}
                  </td>
                  <td className="px-4 py-3">{pi.startedBy ?? '—'}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>
    </div>
  );
}
