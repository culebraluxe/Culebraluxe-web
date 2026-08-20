export const fundingSchema = {
  type: 'object',
  required: ['fundingReference', 'fundingDate'],
  properties: {
    fundingReference: {
      type: 'string',
      title: 'Funding Reference / Transaction ID',
    },
    fundingDate: {
      type: 'string',
      title: 'Funding Date',
      format: 'date',
    },
    notes: {
      type: 'string',
      title: 'Operations Notes',
    },
  },
} as const;

export const fundingUiSchema = {
  notes: { 'ui:widget': 'textarea', 'ui:options': { rows: 3 } },
};
