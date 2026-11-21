# Done

- **What was built**: `ContactForm::subject()` now strips control characters
  (CR/LF/NUL/etc.) from the user-supplied `name` and `email` before building
  the subject, so a newline in either field can no longer make Resend reject
  the send with HTTP 422. Clean input is byte-for-byte unchanged.
- **Commit SHA(s)**: c2218b4
- **Verification**: `./scripts/test.sh` → 🎉 SUCCESS (133/133 tests pass,
  including 6 new subject-sanitization unit tests; fmt/clippy/check clean).
- **Reviewer findings**: No blockers. Two P2 nits, both explicitly within the
  task's allowed "strip or collapse" range: (1) `char::is_control()` covers
  Unicode `Cc` but not `U+2028`/`U+2029` line/paragraph separators — not CR/LF
  and not header line breaks in this path; (2) stripping deletes rather than
  joins-with-space, so `"Alan\nVardy"` → `"AlanVardy"`. `body()` deliberately
  keeps raw values (text body may contain newlines). No fixes required.
- **Remaining manual items**: None. (Note: an unrelated scaffold placeholder
  `DELETEME` deletion is present in the working tree from the branch start
  commit and was left untouched.)
