export const seniorApprovalSchema = {
  type: 'object',
  required: ['decision'],
  properties: {
    decision: {
      type: 'string',
      title: 'Senior Decision',
      enum: ['approve', 'reject'],
      enumNames: ['Approve', 'Reject'],
    },
    overrideReason: {
      type: 'string',
      title: 'Reason for override / decision',
    },
    approvedAmount: {
      type: 'number',
      title: 'Approved Amount (if different)',
    },
  },
} as const;

export const seniorApprovalUiSchema = {
  decision: { 'ui:widget': 'radio' },
  overrideReason: { 'ui:widget': 'textarea', 'ui:options': { rows: 4 } },
};
