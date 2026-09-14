# ENG-FORGE-ARCH-01 — write down the workflow architecture (simple → advanced → PhD)

## Why

The engine's real knowledge lived in three places that all evaporate: commit messages, story
packets, and the head of whoever was debugging at 2am. Nine defects were diagnosed in one day
(2026-09-13) and every one of them reduced to a LAW — a refusal that did not name its cause, a
value computed and never wired to its reader, an environment with an implicit default, a publish
that could never succeed. None of those laws were written anywhere, so the next session would have
re-derived them at the same cost.

A factory whose operating principles exist only as folklore is one context reset away from being a
mystery again.

## Units

- **A** — one durable document, `docs/agent/WORKFLOW-ARCHITECTURE.md`, layered so a newcomer can
  climb it: Part 0 (one paragraph) → Part I (simple: three layers, six roles, normal delivery) →
  Part II (advanced: the machine as built, with file names) → Part III (the laws) → Parts IV–X
  (failure classification, V7 roadmap, CrewAI review, punch-list, swarm doctrine, diagnostic
  ladder, glossary).
- **B** — every claim is labelled **[built]**, **[measured]**, or **[proposed]**. A design that is
  not implemented must never read like one that is; that distinction is the whole value of the doc.
- **C** — it records the traps, not just the design: fast-forward-only publish, the DEV default,
  the parallel-dimension screen, three names for one privilege set, `workflowInstanceId`-scoped
  claims. A doc that lists only the target state teaches nothing about the minefield.
- **D** — the author's V7 list, CrewAI review, P0/P1 punch-list and swarm doctrine are carried in
  with their own status labels, so the design thinking is not lost when the session ends.

## Acceptance

- A reader who has never seen the engine can explain, after Part I, what the three layers are and
  why the XML must not name a provider.
- A reader about to change the engine can name the law they are at risk of violating, and find the
  file that implements it.
- No statement in the document can be mistaken for implemented work when it is not.

## Evidence (2026-09-13)

- `docs/agent/WORKFLOW-ARCHITECTURE.md` (new, ~580 lines, 10 parts).
- Pointers added from `docs/agent/CURRENT.md` and `docs/agent/MEMORY.md` so the doc is reachable
  from the two files an agent already loads.
- Source material: nine defects fixed on 2026-09-13 (`2cf30c7` publish integration, `f74e406`
  environment badge, `bef7c14` QA reason capture, `95673f9` acceptance gate + `first_viol`,
  `b3a6367` nav retirement) and migrations 174/175/176.

## Not this story

- Not a V7 implementation. Part V and Part VII are recorded design, not delivered features.
- Not a CrewAI migration, and not a recommendation to start one.
- Not an engine change: no behaviour is modified by this story.
