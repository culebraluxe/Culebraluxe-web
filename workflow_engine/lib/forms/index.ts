import { creditReviewSchema, creditReviewUiSchema } from './credit-review';
import { seniorApprovalSchema, seniorApprovalUiSchema } from './senior-approval';
import { fundingSchema, fundingUiSchema } from './funding';

export const formRegistry = {
  'credit-review-form': {
    schema: creditReviewSchema,
    uiSchema: creditReviewUiSchema,
  },
  'senior-approval-form': {
    schema: seniorApprovalSchema,
    uiSchema: seniorApprovalUiSchema,
  },
  'funding-form': {
    schema: fundingSchema,
    uiSchema: fundingUiSchema,
  },
} as const;

export type FormKey = keyof typeof formRegistry;
