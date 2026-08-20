import { StartProcessForm } from '@/components/workflow/StartProcessForm';

export default function StartProcessPage() {
  return (
    <div className="max-w-2xl mx-auto py-10 px-4">
      <h1 className="text-2xl font-bold mb-6">Start New Process</h1>
      <StartProcessForm />
    </div>
  );
}
