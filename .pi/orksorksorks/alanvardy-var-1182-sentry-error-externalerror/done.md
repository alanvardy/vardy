# Done

- **What was built**: Two-part fix.
  1. `ContactForm::subject()` strips control characters (CR/LF/NUL/etc.) from
     the user-supplied `name` and `email` before building the subject, so a
     newline can never make Resend reject the send with HTTP 422. Clean input
     is byte-for-byte unchanged.
  2. `ContactForm::looks_like_bot()` flags CR/LF in `name`/`email` (single-line
     fields where a line break is never legitimate, and CRLF is the classic
     SMTP header-injection probe). The contact handler drops such submissions
     silently — renders the thank-you page, sends nothing — like the honeypot.
     `message` is excluded (multi-line is legitimate); the subject sanitizer
     remains as defense-in-depth for other control characters.
- **Commit SHA(s)**: c2218b4 (part 1), b1884ed (part 2)
- **Verification**: `./scripts/test.sh` → 🎉 SUCCESS (139/139 tests pass,
  including 6 subject-sanitization unit tests, 4 `looks_like_bot` unit tests,
  and 2 handler tests for CR/LF drops; fmt/clippy/check clean).
- **Reviewer findings**: No blockers. Two P2 nits, both explicitly within the
  task's allowed "strip or collapse" range: (1) `char::is_control()` covers
  Unicode `Cc` but not `U+2028`/`U+2029` line/paragraph separators — not CR/LF
  and not header line breaks in this path; (2) stripping deletes rather than
  joins-with-space, so `"Alan\nVardy"` → `"AlanVardy"`. `body()` deliberately
  keeps raw values (text body may contain newlines). No fixes required.
- **Remaining manual items**: The CR/LF drop is intentionally aggressive — a
  human who pastes a multi-line name/email will be silently dropped. Revisit
  measuring real vs. bot traffic if that proves costly. (Note: an unrelated
  scaffold placeholder
  `DELETEME` deletion is present in the working tree from the branch start
  commit and was left untouched.)
