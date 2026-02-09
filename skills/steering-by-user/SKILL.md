---
name: steering-by-user
description: Use when the human gives any steering (including soft preferences or suggestions) or when you infer a likely preference from feedback, and you should log and consolidate it so future tasks follow it consistently.
---

# Steering By User

## Overview
Turn in-the-moment user steering into explicit, reviewable rules, persist them, and apply them immediately (and in future tasks) without re-litigating.

## When To Use

- The user gives explicit steering: “from now on…”, “always…”, “never…”, “stop doing…”.
- The user gives *soft* steering: “I prefer…”, “can you…”, “it’d be nice if…”, “maybe…”, “suggestion: …”.
- The user corrects your process/output (too many questions, wrong defaults, wrong format, etc.).
- You infer a likely preference from strong feedback (positive or negative) that would change future behavior.
- You made a repeat mistake and the fix is “behavioral” (process/style), not code.

## Pressure Scenarios (Tests)

1. User: “Don’t ask permission to run commands; just do it.”
Expected: You stop asking for approval, and you update `STEERING.md` so it sticks.

2. User: “When I say ‘push’, I mean commit and push.”
Expected: You treat “push” as commit+push unless there are staged/unstaged ambiguities; you document that rule.

3. User: “Keep answers short; no fluff.”
Expected: You respond concisely and update `STEERING.md` with a concrete rule (“no openers”, “no cheerleading”).

4. User: “It’s fine if you do X, but I’d rather you do Y.”
Expected: You treat this as steering, log it, and consolidate it into an existing rule if one exists.

## Procedure

1. Extract steering as crisp rules:
Write 1-5 “Do/Don’t” bullets, each concrete enough to follow without interpretation.

2. Be permissive by default:
If it smells like a preference, log it.
Only ask a question when you would otherwise record the wrong rule.

3. Persist + consolidate:
Update `STEERING.md` at the repo root.
Before adding a new bullet, scan for an existing rule that can be amended instead of duplicated.

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
