import { engine } from '@/lib/workflow/db';
import { TaskList } from '@/components/workflow/TaskList';

export default async function TasksPage() {
  const currentUserId = 'john.doe';
  const tasks = await engine.getActiveTasksForUser(currentUserId);

  return (
    <div className="max-w-4xl mx-auto py-10 px-4">
      <h1 className="text-2xl font-bold mb-6">My Tasks</h1>
      <TaskList tasks={tasks} currentUserId={currentUserId} />
    </div>
  );
}
