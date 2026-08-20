import { engine } from '@/lib/workflow/db';
import { TaskForm } from '@/components/workflow/TaskForm';
import { notFound } from 'next/navigation';

export default async function TaskPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  const task = await engine.getTask(id);

  if (!task) notFound();

  const currentUserId = 'john.doe';

  return (
    <div className="max-w-4xl mx-auto py-10 px-4">
      <TaskForm task={task} currentUserId={currentUserId} />
    </div>
  );
}
