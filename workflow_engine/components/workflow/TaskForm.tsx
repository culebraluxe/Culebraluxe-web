'use client';

import { useState } from 'react';
import Form from '@rjsf/core';
import validator from '@rjsf/validator-ajv8';
import { formRegistry, FormKey } from '@/lib/forms';
import { completeTaskAction, claimTaskAction } from '@/app/actions/workflow';

interface TaskFormProps {
  task: {
    id: string;
    name: string;
    formKey?: string | null;
    status: string;
    assignee?: string | null;
    formData?: Record<string, any>;
  };
  currentUserId: string;
  onCompleted?: () => void;
}

export function TaskForm({ task, currentUserId, onCompleted }: TaskFormProps) {
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const formConfig = task.formKey ? formRegistry[task.formKey as FormKey] : null;

  async function handleClaim() {
    setSubmitting(true);
    setError(null);
    try {
      await claimTaskAction(task.id, currentUserId);
      onCompleted?.();
    } catch (err: any) {
      setError(err.message);
    } finally {
      setSubmitting(false);
    }
  }

  async function handleSubmit(data: { formData: Record<string, any> }) {
    setSubmitting(true);
    setError(null);
    try {
      const transitionName = data.formData.decision ?? undefined;
      await completeTaskAction({
        taskId: task.id,
        userId: currentUserId,
        formData: data.formData,
        transitionName,
      });
      onCompleted?.();
    } catch (err: any) {
      setError(err.message);
    } finally {
      setSubmitting(false);
    }
  }

  if (task.status === 'ready' && task.assignee !== currentUserId) {
    return (
      <div className="border rounded-lg p-6 bg-white shadow-sm">
        <h2 className="text-xl font-semibold mb-2">{task.name}</h2>
        <p className="text-gray-600 mb-4">This task is available to be claimed.</p>
        {error && <p className="text-red-600 mb-3">{error}</p>}
        <button
          onClick={handleClaim}
          disabled={submitting}
          className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-50"
        >
          {submitting ? 'Claiming…' : 'Claim Task'}
        </button>
      </div>
    );
  }

  if (task.status === 'completed') {
    return (
      <div className="border rounded-lg p-6 bg-gray-50">
        <h2 className="text-xl font-semibold mb-2">{task.name}</h2>
        <p className="text-green-700">Task completed</p>
        <pre className="mt-3 text-sm bg-white p-3 rounded overflow-auto">
          {JSON.stringify(task.formData, null, 2)}
        </pre>
      </div>
    );
  }

  if (!formConfig) {
    return <div className="text-red-600">No form configured for this task.</div>;
  }

  return (
    <div className="border rounded-lg p-6 bg-white shadow-sm max-w-2xl">
      <h2 className="text-xl font-semibold mb-4">{task.name}</h2>
      {error && (
        <div className="mb-4 p-3 bg-red-50 text-red-700 rounded">{error}</div>
      )}
      <Form
        schema={formConfig.schema as any}
        uiSchema={formConfig.uiSchema}
        formData={task.formData ?? {}}
        validator={validator}
        onSubmit={handleSubmit}
        disabled={submitting}
      >
        <div className="mt-6 flex gap-3">
          <button
            type="submit"
            disabled={submitting}
            className="px-4 py-2 bg-green-600 text-white rounded hover:bg-green-700 disabled:opacity-50"
          >
            {submitting ? 'Submitting…' : 'Complete Task'}
          </button>
        </div>
      </Form>
    </div>
  );
}
