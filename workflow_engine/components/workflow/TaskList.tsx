'use client';

import Link from 'next/link';
import { Task } from '@/lib/workflow/types';

interface TaskListProps {
  tasks: Task[];
  currentUserId: string;
}

export function TaskList({ tasks }: TaskListProps) {
  if (tasks.length === 0) {
    return <p className="text-gray-500">No active tasks.</p>;
  }

  return (
    <div className="space-y-3">
      {tasks.map((task) => (
        <Link
          key={task.id}
          href={`/tasks/${task.id}`}
          className="block border rounded-lg p-4 hover:bg-gray-50 transition bg-white"
        >
          <div className="flex justify-between items-start">
            <div>
              <h3 className="font-medium">{task.name}</h3>
              <p className="text-sm text-gray-500 mt-1">
                Status: <span className="capitalize">{task.status}</span>
                {task.assignee && ` · Assigned to ${task.assignee}`}
              </p>
            </div>
            <div className="text-sm text-gray-400">
              {task.priority > 0 && (
                <span className="mr-3">Priority {task.priority}</span>
              )}
              {new Date(task.createdAt).toLocaleDateString()}
            </div>
          </div>
        </Link>
      ))}
    </div>
  );
}
