# Task

Add two missing repo-root config files flagged by the codebase-health audit
(REMEDIATION-16: both currently FAIL — neither file exists in the tree).

1. **`.editorconfig`** (new, repo root):
   - `indent_style = space`, `indent_size = 4` for `*.rs`; `indent_size = 2`
     for yaml/toml/md
   - `charset = utf-8`, `end_of_line = lf`
   - `insert_final_newline = true`, `trim_trailing_whitespace = true`

2. **`.gitattributes`** (new, repo root):
   - `* text=auto eol=lf` so text files normalize to LF
   - Mark binary assets (`static/*.jpg`, `*.png`, `*.svg`) as binary
     (`-text` / `binary`) to avoid diff/EOL noise
   - No LFS — repo hasn't grown enough to warrant it; skip unless asked

Audit evidence requires these exact files; content is dictated by the ticket
and follows the standard formats.

## Why SMALL

A: two net-new files at the repo root, no module touched, ≤3 files; B: zero
unknowns — the ticket specifies exact content, indent sizes, and the LFS
answer (no); C: no schema/migration; D: no subsystem, no existing code
modified; E: no design decision or sign-off needed; F: config files only, no
test surface affected.

## Key files

- `.editorconfig` (new, repo root)
- `.gitattributes` (new, repo root)

Session-state caution: HEAD commit `4c8fe15` is *titled* "Add .editorconfig
and .gitattributes" but only added a stray `DELETEME` file — neither config
file exists in the tree. The working tree also has an unstaged deletion of
`DELETEME`. Treat all of this as an orphan from an interrupted session: verify
current branch state before editing, and delete/revert the stray `DELETEME`
file (compare against the ticket — it is not part of this task). Before
finalizing `eol=lf`, check the tracked text files for CRLF so the new
`.gitattributes` doesn't create a surprise renormalization diff.