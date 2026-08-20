'use client';

import { useState } from 'react';
import { useRouter } from 'next/navigation';
import { startProcessAction } from '@/app/actions/workflow';

export function StartProcessForm() {
  const router = useRouter();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleSubmit(e: React.FormEvent<HTMLFormElement>) {
    e.preventDefault();
    setLoading(true);
    setError(null);

    const formData = new FormData(e.currentTarget);

    try {
      const result = await startProcessAction(formData);
      router.push(`/processes/${result.processInstanceId}`);
    } catch (err: any) {
      setError(err.message || 'Failed to start process');
    } finally {
      setLoading(false);
    }
  }

  return (
    <form onSubmit={handleSubmit} className="space-y-6 border rounded-lg p-6 bg-white shadow-sm">
      {error && (
        <div className="p-3 bg-red-50 text-red-700 rounded">{error}</div>
      )}

      <div>
        <label className="block text-sm font-medium mb-1">Process Definition</label>
        <select
          name="definitionKey"
          required
          className="w-full border rounded px-3 py-2"
          defaultValue="loan-approval"
        >
          <option value="loan-approval">Loan Approval</option>
        </select>
      </div>

      <div>
        <label className="block text-sm font-medium mb-1">Business Key (optional)</label>
        <input
          name="businessKey"
          type="text"
          placeholder="e.g. LOAN-2026-00482"
          className="w-full border rounded px-3 py-2"
        />
      </div>

      <div>
        <label className="block text-sm font-medium mb-1">Amount</label>
        <input
          name="amount"
          type="number"
          required
          min={1}
          className="w-full border rounded px-3 py-2"
          placeholder="250000"
        />
      </div>

      <div>
        <label className="block text-sm font-medium mb-1">Customer ID</label>
        <input
          name="customerId"
          type="text"
          required
          className="w-full border rounded px-3 py-2"
          placeholder="C-99821"
        />
      </div>

      <div>
        <label className="block text-sm font-medium mb-1">Started By</label>
        <input
          name="startedBy"
          type="text"
          required
          defaultValue="john.doe"
          className="w-full border rounded px-3 py-2"
        />
      </div>

      <button
        type="submit"
        disabled={loading}
        className="w-full px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-50"
      >
        {loading ? 'Starting…' : 'Start Process'}
      </button>
    </form>
  );
}
