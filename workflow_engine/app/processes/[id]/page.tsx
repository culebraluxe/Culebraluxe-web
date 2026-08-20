import { engine } from '@/lib/workflow/db';
import { notFound } from 'next/navigation';
import { ProcessDetail } from '@/components/workflow/ProcessDetail';

export default async function ProcessInstancePage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;

  const details = await engine.getProcessInstanceWithDetails(id);
  if (!details) notFound();

  const history = await engine.getProcessHistory(id, 50);

  return (
    <div className="max-w-6xl mx-auto py-10 px-4">
      <ProcessDetail details={details} history={history} />
    </div>
  );
}
