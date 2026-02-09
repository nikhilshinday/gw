---
name: steering-by-user
description: Use when the human gives behavioral steering, style preferences, or corrections and you need to make that guidance durable and consistently enforced across future tasks.
---

# Steering By User

## Overview
Turn in-the-moment user steering into explicit, reviewable rules, persist them, and apply them immediately (and in future tasks) without re-litigating.

## When To Use

- The user says “from now on…”, “always…”, “never…”, “stop doing…”.
- The user corrects your process (too many questions, wrong default, wrong output format, etc.).
- You made a repeat mistake and the fix is “behavioral” (process/style), not code.

## Pressure Scenarios (Tests)

1. User: “Don’t ask permission to run commands; just do it.”
Expected: You stop asking for approval, and you update `STEERING.md` so it sticks.

2. User: “When I say ‘push’, I mean commit and push.”
Expected: You treat “push” as commit+push unless there are staged/unstaged ambiguities; you document that rule.

3. User: “Keep answers short; no fluff.”
Expected: You respond concisely and update `STEERING.md` with a concrete rule (“no openers”, “no cheerleading”).

## Procedure

1. Extract steering as crisp rules:
Write 1-5 “Do/Don’t” bullets, each concrete enough to follow without interpretation.

2. Confirm only what’s ambiguous:
Ask a single yes/no or multiple-choice question if the steering could be interpreted two ways.
If it’s unambiguous, do not ask; proceed.

3. Persist:
Update `STEERING.md` at the repo root.
Prefer adding to an existing section; otherwise add a new section with a clear heading.

4. Make it actionable:
Add one example (before/after phrasing or a command) when it reduces future ambiguity.

5. Apply immediately:
Adjust your behavior in the current thread based on the updated rules.

6. Log significant steering:
If the steering changes default behavior in a way humans will notice, add a short entry in `AGENTS.md` describing what changed and why.

## Common Mistakes

- Vague steering like “be better”: rewrite into observable behavior (“Answer in <= 6 lines unless asked”, “Prefer `rg` over `grep`”).
- Over-collecting: don’t turn one preference into a big policy document.
- Storing duplicates: merge with existing rules instead of adding near-identical bullets.

