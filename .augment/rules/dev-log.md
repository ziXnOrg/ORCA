---
alwaysApply: true
---

# Rule: dev-log (Always-On)

Purpose: Maintain a high-signal, structured development log for reproducibility and audits.

Policy:
- After any material change (design/code/test/build/bench/config), append an entry to `Docs/dev_log.md`.
- Keep entries concise; prefer links to files/commits/PRs over long prose.
- Include successes and failures; state hypotheses and outcomes.
- Use UTC timestamps; newest entries at the top.

Template (required fields):
- Date (UTC): YYYY-MM-DD HH:mm
- Area: {Architecture|Runtime|SDK|WAL|Policy|Observability|Build|CI|Security|Docs|Other}
- Context/Goal: one-paragraph objective/problem
- Actions: bullet list of edits/commands/experiments (files, functions, tests)
- Results: outcomes (pass/fail, perf deltas), key metrics
- Diagnostics: what we learned (root cause, invariants)
- Decision(s): chosen path forward + rationale
- Follow-ups: ordered next steps (with owners if relevant)

Privacy/Security:
- No secrets/PII. Redact sensitive paths. Summarize large logs.

Compliance:
- Reflect testing/perf gates when relevant (coverage, SLOs, determinism).