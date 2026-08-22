# Research: Best Practices for Writing AGENTS.md Files (2025–2026)

## Summary
AGENTS.md has become the de facto cross-tool standard for agent instruction files — an open spec stewarded by the Agentic AI Foundation under the Linux Foundation, natively read by 30+ tools including OpenAI Codex, Cursor, Google Jules, Gemini CLI, GitHub Copilot, Windsurf, Aider, Amp, and Devin. Consensus across vendor docs and community analysis: keep files short (20–150 lines), lead with exact executable commands, prefer concrete/prohibitive rules over aspirational prose, layer instructions global → repo root → nested directories with "nearest file wins," and enforce critical rules with hooks/linters/CI rather than prose alone. Global files (`~/.codex/AGENTS.md`, `~/.claude/CLAUDE.md`) should hold personal, universal working preferences; per-repo files hold team/architecture/build rules; nested files override for their subtree.

## Findings

### 1. Vendor guidance

1. **OpenAI Codex** — Official docs describe AGENTS.md as plain Markdown with no required schema. Codex builds an instruction chain per session: global (`~/.codex/AGENTS.override.md`, else `~/.codex/AGENTS.md`), then walks from Git root down to cwd, concatenating root→cwd so **files closer to the working directory override earlier guidance**. Hard limit: `project_doc_max_bytes` caps combined instructions at **32 KiB (~8k tokens) by default**, silently dropping overflow — a strong argument for lean files and nested splitting. Official triggers to update the file: repeated mistakes, agent reading too many files, recurring PR feedback; pair prose with pre-commit hooks/linters/typecheckers ("enforcement infrastructure"), and delegate updates via `@codex add this to AGENTS.md`. [Codex docs](https://learn.chatgpt.com/docs/agent-configuration/agents-md), [Codex CLI custom instructions](https://mintlify.wiki/openai/codex/advanced/custom-instructions)

2. **Anthropic Claude Code** — Claude Code reads `CLAUDE.md`, **not** AGENTS.md natively. Memory layers: org-managed → user (`~/.claude/CLAUDE.md`) → project (`CLAUDE.md` / `.claude/CLAUDE.md`) → local personal (`CLAUDE.local.md`, gitignored) → modular `.claude/rules/*.md`. Ancestor-directory CLAUDE.md files load at startup; subdirectory files load lazily on first read of that subtree. Official advice: keep each file **under ~200 lines**; ask of every line *"would removing this cause mistakes?"*; use imperative, verifiable language; prohibitions beat suggestions. Standard bridge to the open standard: a one-line `@AGENTS.md` import at the top of CLAUDE.md (or symlink). Critical always-must-happen behavior belongs in **PreToolUse hooks or CI, not memory files** — "Claude can't forget a hook." [Memory docs](https://code.claude.com/docs/en/memory), [Help Center](https://support.claude.com/en/articles/14553240-give-claude-context-claude-md-and-better-prompts)

3. **Cursor** — Natively reads root `AGENTS.md` as an "Always" rule (full contents load every request); since Cursor 3.6 it discovers **nested AGENTS.md files across the whole repo** with nearest-file precedence. For finer control, `.cursor/rules/*.mdc` files support frontmatter scoping (`description` for intelligent apply, `globs` for path-scoped apply, `alwaysApply`). Precedence: Team Rules > Project Rules > User Rules; legacy `.cursorrules` is deprecated. Official guidance: keep rules under ~500 lines total, use the weakest reliable trigger mode, reference files with `@path` instead of copying content. Caution: hundreds of always-on nested AGENTS.md in a monorepo can blow past context limits. [Cursor rules docs](https://cursor.com/docs/rules)

4. **Google Jules** — Automatically reads AGENTS.md from the repo root to understand tools/conventions and generate better plans; docs simply advise keeping it up to date. Older `JULES.md` references are superseded. [Jules docs](https://jules.google/docs)

### 2. Recommended hierarchy — what belongs where

Decision rule of thumb (consistent across Codex docs, Claude Code docs, and community guides):

| Layer | Location | Belongs there |
|---|---|---|
| **Global / personal** | `~/.codex/AGENTS.md`, `~/.claude/CLAUDE.md` | Personal preferences applying to *every* repo: package-manager choice, communication style/verbosity, "ask before destructive git/db commands," commit-message habits, clarifying-question defaults. Not committed to git — so never put team rules here. [agentpatterns layered scopes](https://github.com/agentpatterns-ai/website/blob/main/instructions/layered-instruction-scopes.md) |
| **Repo root** | `<repo>/AGENTS.md` (committed) | Team-shared: build/test/lint commands, architecture overview (~3 sentences + module map), code style conventions, testing requirements, boundaries ("never edit generated/", "all changes via PR"), PR/commit workflow. |
| **Nested directory** | `<subdir>/AGENTS.md` | Subsystem-specific rules: module layout, domain-specific test gates, scoped constraints. Nearest-file-wins by position in the prompt. Use for monorepos instead of one giant root file. |

Pitfall: global and project layers are concatenated, and combined size caps (32 KiB in Codex) apply to the *total* — deep hierarchies can silently truncate. Keep every layer small.

### 3. Community best practices (2025–2026)

GitHub's analysis of **2,500+ AGENTS.md repos** found the #1 failure mode is vagueness, and identified four patterns of effective files ([github.blog](https://github.blog/ai-and-ml/github-copilot/how-to-write-a-great-agents-md-lessons-from-over-2500-repositories/?lid=1qdHhBbRrqG2FuTpZ)):

1. **Executable commands, early** — exact invocations with flags (`cargo clippy -- -D warnings`, `./scripts/test.sh`), covering build/test/lint/run/single-test. Not "run the tests."
2. **Code over prose for style** — one correct snippet (or correct-vs-wrong pair) beats paragraphs.
3. **Tiered boundaries** — ✅ Always / ⚠️ Ask first / 🚫 Never (e.g., never commit secrets — the single most common helpful constraint found).
4. **Specialist scoping** — narrow named roles/subagents with explicit exclusions, added only after observed mistakes.

Recommended sections (convergent across guides): project overview (1–3 sentences + stack specifics), Commands, Architecture/layout, Code style, Testing, Boundaries/security, Git/PR workflow, common pitfalls, do-not-touch paths. **Excluded:** general programming advice agents already know, full API docs (link instead), secrets, changelogs, anything obvious from the file tree, aspirational rules the team doesn't follow.

Length consensus varies slightly by source: **20–100 lines ideal** ([AAIF/morphllm](https://www.morphllm.com/agents-md-guide)), **<150 lines** ([agents.md ecosystem guidance](https://jules.google/docs)), **<200 lines hard ceiling** ([Claude Code docs](https://code.claude.com/docs/en/memory)). All agree adherence degrades with length and autogenerated exhaustive files can hurt performance.

Anti-patterns to avoid: contradictions between layers (later-positioned nested text wins only by recency — don't rely on it), stale info (prefer linking living docs; use pointer pattern — AGENTS.md as table-of-contents into `docs/`), duplication of README/upstream docs, hardcoded absolute paths, vague aspirational rules, and stuffing everything global until unreadable. Prefer fixing recurring issues at the enforcement level (lint rule, CI check, pre-commit hook) over adding another prose rule. Security note: researchers documented "Rules File Backdoor" injection attacks against these files since they're injected into prompts — review AGENTS.md PRs like code.

Empirical backing: AAIF's controlled benchmark (5 runs × conditions, GitHub Copilot CLI) measured a 12-line AGENTS.md delivering **27% less wall time, 24% fewer credits, 26% smaller diffs** on an ambiguous task; concrete instructions measurably outperformed aspirational ones. [AAIF blog](https://aaif.io/blog/measuring-agents-md-what-five-runs-show-that-one-doesn-t)

### 4. Universal rules vs. environment config in layered/global files

Consensus: split by **ownership and scope, not by category**:
- **Global files = universal personal working agreements**: how the agent communicates with you (concise responses, ask clarifying questions when unclear), personal hygiene gates you want everywhere (never push to main, run tests before declaring done, commit message style), personal security habits, preferred tools. These are fine as universal rules precisely because they're yours and version-control-free.
- **Per-repo files = environment/team config**: exact toolchain commands, framework-specific conventions, architecture boundaries, migration policies. A Rust axum app's `sqlx migrate add` rule or iOS app's build scheme belongs here, never global.
- One nuance: *testing gates* sit awkwardly — "always run the project's test command before finishing" is a good universal rule; the *specific* command (`./scripts/test.sh`) belongs per-repo. Phrase global rules generically and let repo files supply the concrete commands.

### 5. Standardization status

The agents.md spec ([agents.md](https://agents.md)) originated May 2025 from OpenAI Codex, Sourcegraph's Amp, Google Jules, Cursor, and Factory; it's now stewarded by the **Agentic AI Foundation under the Linux Foundation** (alongside MCP, goose, agentgateway). As of early 2026: **30+ tools read it natively** and **60,000+ open-source repos** use it. Notable exception remains Claude Code (bridge via `@AGENTS.md` import or symlink). GitHub Copilot reads both `.github/copilot-instructions.md` and AGENTS.md additively. Ecosystem positioning: AGENTS.md is the static project-context layer, complementary to MCP (dynamic tools) and Agent Skills/SKILL.md (on-demand task knowledge). Practical implication: maintain **one canonical AGENTS.md per repo level** and make CLAUDE.md/GEMINI.md thin overlays that import it, guarding drift with a CI check if desired.

## Application to the user's setup
- The existing `~/AGENTS.md` home-conventions file matches best practice for global scope (shell env facts like fish syntax, Bear CLI path, sqlx migration policy are genuinely machine-level). Keep team/framework rules out of it.
- Per-project files (axum web apps, iOS Swift, Rust CLI) already carry correct repo-level content (module layout, test gates, QRSPI pipeline). Watch length: several sections (error responses, tests, migrations) could be tightened toward concrete command examples.
- If any repo uses Claude Code alongside other agents, add the `@AGENTS.md` import bridge.
- Consider moving "must-always-happen" gates (format/test before done, never push to main) partially into hooks/CI (`scripts/test.sh` is already close to this pattern).

## Sources
- Kept: [OpenAI Codex — Custom instructions with AGENTS.md](https://learn.chatgpt.com/docs/agent-configuration/agents-md) — authoritative discovery/merge mechanics, 32 KiB cap, official update triggers
- Kept: [Claude Code — Memory docs](https://code.claude.com/docs/en/memory) — authoritative memory hierarchy, <200-line guidance, hooks-over-prose principle
- Kept: [Claude Help Center — CLAUDE.md and better prompts](https://support.claude.com/en/articles/14553240-give-claude-context-claude-md-and-better-prompts) — official writing heuristics
- Kept: [Cursor Rules docs](https://cursor.com/docs/rules) — AGENTS.md support, .mdc scoping, precedence, monorepo context caution
- Kept: [Google Jules docs](https://jules.google/docs) — vendor confirmation of AGENTS.md auto-discovery
- Kept: [GitHub Blog — Lessons from 2,500+ AGENTS.md repos](https://github.blog/ai-and-ml/github-copilot/how-to-write-a-great-agents-md-lessons-from-over-2500-repositories/?lid=1qdHhBbRrqG2FuTpZ) — largest empirical corpus, four effective patterns
- Kept: [AAIF — Measuring AGENTS.md benchmark](https://aaif.io/blog/measuring-agents-md-what-five-runs-show-that-one-doesn-t) — quantified benefit, concrete>aspirational evidence
- Kept: [agents.md](https://agents.md) — canonical open spec
- Kept: [morphllm AGENTS.md Spec guide (2026)](https://www.morphllm.com/agents-md-guide) — adoption counts, tool compatibility list, section recommendations
- Kept: [agentpatterns — Layered Instruction Scopes](https://github.com/agentpatterns-ai/website/blob/main/instructions/layered-instruction-scopes.md) — what belongs at each hierarchy level
- Dropped: SEO/aggregator clones (codex-docs.com, agentsmd.io, apidog blog) — derivative of primary sources
- Dropped: Non-English mirrors and third-party tutorial repos — redundant with official docs

## Gaps
- No official Apple/Xcode-side guidance exists (no vendor AGENTS.md story for iOS); Swift-app advice is community-only.
- Exact current default `project_doc_max_bytes` and fallback-filename behavior may vary by Codex CLI version; verify against installed version if relying on the cap.
- pi coding agent's own AGENTS.md discovery semantics were not covered by public vendor docs found in this pass; confirm from its local skill/docs if precision matters.
