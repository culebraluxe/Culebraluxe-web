-- Seed the loan-approval process definition
INSERT INTO process_definitions (key, version, name, definition, status, created_by)
VALUES (
  'loan-approval',
  1,
  'Loan Approval',
  '{
    "startNodeId": "start",
    "nodes": {
      "start": {
        "id": "start",
        "type": "start",
        "transitions": [{ "name": "to_review", "to": "credit_review" }]
      },
      "credit_review": {
        "id": "credit_review",
        "type": "task",
        "name": "Credit Review",
        "formKey": "credit-review-form",
        "candidateGroups": ["credit-officers", "john.doe"],
        "priority": 10,
        "transitions": [
          { "name": "approve", "to": "amount_decision" },
          { "name": "reject", "to": "rejected_end" }
        ]
      },
      "amount_decision": {
        "id": "amount_decision",
        "type": "decision",
        "decisions": [
          { "condition": "amount > 100000", "transition": "senior_approval" },
          { "condition": "amount <= 100000", "transition": "auto_approve" }
        ],
        "transitions": [
          { "name": "senior_approval", "to": "senior_review" },
          { "name": "auto_approve", "to": "funding" }
        ]
      },
      "senior_review": {
        "id": "senior_review",
        "type": "task",
        "name": "Senior Credit Approval",
        "formKey": "senior-approval-form",
        "candidateGroups": ["senior-credit", "john.doe"],
        "priority": 20,
        "transitions": [
          { "name": "approve", "to": "funding" },
          { "name": "reject", "to": "rejected_end" }
        ]
      },
      "funding": {
        "id": "funding",
        "type": "task",
        "name": "Initiate Funding",
        "formKey": "funding-form",
        "candidateGroups": ["operations", "john.doe"],
        "transitions": [{ "name": "complete", "to": "approved_end" }]
      },
      "approved_end": {
        "id": "approved_end",
        "type": "end",
        "name": "Approved"
      },
      "rejected_end": {
        "id": "rejected_end",
        "type": "end",
        "name": "Rejected"
      }
    }
  }'::jsonb,
  'active',
  'admin'
);
