'use server';

import { engine } from '@/lib/workflow/db';
import { revalidatePath } from 'next/cache';

export async function startProcessAction(formData: FormData) {
  const definitionKey = formData.get('definitionKey') as string;
  const startedBy = formData.get('startedBy') as string;
  const businessKey = (formData.get('businessKey') as string) || undefined;
  const amount = Number(formData.get('amount'));
  const customerId = formData.get('customerId') as string;

  const result = await engine.startProcess({
    definitionKey,
    startedBy,
    businessKey,
    variables: { amount, customerId },
  });

  revalidatePath('/processes');
  revalidatePath('/dashboard');
  return result;
}

export async function claimTaskAction(taskId: string, userId: string) {
  await engine.claimTask(taskId, userId);
  revalidatePath('/tasks');
  revalidatePath('/dashboard');
}

export async function completeTaskAction(data: {
  taskId: string;
  userId: string;
  formData?: Record<string, any>;
  transitionName?: string;
}) {
  await engine.completeTask(data);
  revalidatePath('/tasks');
  revalidatePath('/processes');
  revalidatePath('/dashboard');
}

export async function signalTokenAction(data: {
  tokenId: string;
  transitionName?: string;
  actor: string;
  variables?: Record<string, any>;
}) {
  await engine.signalToken(data);
  revalidatePath('/processes');
}
