# VAR-676: AGENTS.md Comparison & Generalization

Comparison of the five agent instruction files, the generalizations applied,
and remaining recommendations.

## Files compared

| File | Project | Stack |
|---|---|---|
| `~/dev/api/AGENTS.md` | api | Rust axum + Postgres/sqlx |
| `~/dev/vardy/AGENTS.md` | vardy | Rust axum + SQLite/sqlx |
| `~/dev/SingleThread/AGENTS.md` | SingleThread | Swift / Xcode iOS + watchOS |
| `~/dev/tod/AGENTS.md` | tod | Rust Todoist CLI |
| `~/AGENTS.md` | home (global) | machine-level conventions |

## What was shared across projects

| Pattern | Before |
|---|---|
| Concise responses + clarifying questions | byte-identical in api & vardy only |
| `./scripts/test.sh` as the pre-commit gate | all 4 code projects, 4 phrasings |
| Happy/sad path tests required | all 4, three phrasings; SingleThread adds failing-test-first for bug fixes |
| Never push directly to main / PR-only | verbatim-identical in api & vardy; tod paraphrased; SingleThread unstated |
| Centralized error handling | all 4 conceptually (`AppError` / `WebError` / `Error`), different mechanics |
| QRSPI phased workflow | all 4 with **three different phase enumerations** |
| Migrations via CLI only | api, vardy, home; "never edit applied migration" only in home |

## Changes made

1. **`~/AGENTS.md` is now the source of truth** for shared rules: Responses,
   Skills locations, Testing (happy/sad + test-gate), Commits/PRs (PR-only,
   comment guidance), canonical QRSPI pipeline
   (`/1_spec → /2_clarify → /3_design → /4_research → /5_plan → /6_implement`),
   error-handling principle, and sqlx migration discipline.
2. **api & vardy**: removed duplicated sections; added "never edit an applied
   migration" (previously only in home); kept project-specific mechanics
   (module layout, arkitect allowlists, `AppError`/`WebError` details,
   ROUTES.md conventions).
3. **SingleThread**: aligned QRSPI wording with the canonical pipeline while
   keeping its design-on-child-branch convention and all Swift/Xcode detail.
4. **tod**: replaced its divergent 6-phase QRS enumeration with the canonical
   pipeline (kept `.pi/qrspi/<issue-id>/` artifacts and per-phase approval
   gates); removed rules now covered globally.

## Drift found but deliberately left per-project

- Error response schema: api mandates `"code"`+`"error"` JSON bodies;
  vardy requires body validation without a fixed schema — mechanics stay local.
- Commit format: tod enforces Conventional Commits (`type/…` branches);
  others are unopinionated.
- Shell: SingleThread/home state fish inline; api/vardy defer to the
  fish-shell skill.

## External best-practice findings

- AGENTS.md is a Linux Foundation–stewarded open spec read natively by 30+
  tools ([agents.md](https://agents.md)); Claude Code bridges via an
  `@AGENTS.md` import ([Claude memory docs](https://code.claude.com/docs/en/memory)).
- Global files should hold personal universal working agreements; repo files
  hold team/toolchain config ([Codex docs](https://learn.chatgpt.com/docs/agent-configuration/agents-md)).
- Keep layers short (<150–200 lines); combined layers can hit Codex's 32 KiB cap.
- Concrete instructions measurably outperform aspirational ones (27% less
  wall time, 26% smaller diffs) ([AAIF benchmark](https://aaif.io/blog/agents-md-what-five-runs-show-that-one-doesn-t));
  GitHub's analysis of 2,500+ repos puts exact executable commands first and
  prefers enforcement via hooks/CI over prose
  ([github.blog](https://github.blog/ai-and-ml/github-copilot/how-to-write-a-great-agents-md-lessons-from-over-2500-repositories/)).

## Remaining recommendations (follow-ups)

- Enforce must-happen gates in hooks/CI rather than prose where possible
  (most repos already have `scripts/test.sh`; consider pre-push hooks).
- If any repo uses Claude Code alongside other agents, add a one-line
  `@AGENTS.md` import to its `CLAUDE.md`.
- Watch total layer size as project files grow; keep each under ~100 lines.
