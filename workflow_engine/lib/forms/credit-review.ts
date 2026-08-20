export const creditReviewSchema = {
  type: 'object',
  required: ['decision', 'riskScore'],
  properties: {
    decision: {
      type: 'string',
      title: 'Decision',
      enum: ['approve', 'reject'],
      enumNames: ['Approve', 'Reject'],
    },
    riskScore: {
      type: 'number',
      title: 'Risk Score (1-100)',
      minimum: 1,
      maximum: 100,
    },
    comment: {
      type: 'string',
      title: 'Comment',
    },
    conditions: {
      type: 'string',
      title: 'Special Conditions (if any)',
    },
  },
} as const;

export const creditReviewUiSchema = {
  decision: { 'ui:widget': 'radio' },
  comment: { 'ui:widget': 'textarea', 'ui:options': { rows: 4 } },
  conditions: { 'ui:widget': 'textarea', 'ui:options': { rows: 3 } },
};
