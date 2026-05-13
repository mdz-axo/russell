# Plan: Empower Jack to Actually Help

## Root Cause Analysis

The three research agents audited every code path in chats, prompts, dispatch, and
help flows. Jack's ability to act is broken by four code bugs and one prompt
weakness:

### Bug 1: Silent ACTION parse failures (HIGHEST IMPACT)
`parse_action_from_response()` returns `None` when any parse step fails — misspelled
skill ID, trailing text on ACTION line, case mismatch, malformed line. No diagnostic
is printed. Jack proposes an action, the operator sees nothing happen, and neither
knows it was dropped. The operator's conversation transcript (".", then "what do you
see?", then "now?", then "can't you run that?") maps directly to this: Jack likely
emitted an ACTION line that failed to parse silently.

**Root location:** `chat.rs:588-626`, 7 `?` operators, all silent.

### Bug 2: Consent gate fires on partial sentences
`is_affirmative()` matches the full trimmed input with `matches!()`. "sure, but check
the CPU first" triggers consent because the whole string "sure, but check the CPU
first" doesn't match any pattern, wait — actually looking at the code again, it does
`trim().to_lowercase()` which becomes `"sure, but check the cpu first"` which does NOT
match `"sure"`. So multi-word inputs DON'T trigger. But "ok" and "yes" are fine as
single words.

Actually the real issue is different: words like "ok" alone trigger execution when the
operator might have intended "ok let me think about it" but hit enter. And "not now",
"later", "wait" silently clear the pending action without any confirmation.

**Root location:** `chat.rs:64-88` and `chat.rs:237-266`.

### Bug 3: Token budget silently drops ACTION lines
`max_tokens: 1024` in `chat.rs:938` is too tight for verbose responses. When Jack's
explanation runs long, the final `ACTION:` line (which must be the last line per
rules) gets truncated. The operator sees Jack's analysis but no action — Jack tried to
help but the system ate his hands.

**Root location:** `chat.rs:938`.

### Bug 4: Objective text only teaches ACTION for interventions
`prompt.rs:198-206` says: "When you identify an intervention... propose it using:
ACTION: <skill-id>/<intervention-id>". It never says "you can also run probes with
ACTION: <skill-id>/<probe-id>". The system prompts (jack.md, jack-chat.md) teach this,
but the user-message objective text (which LLMs weight heavily as "current task
instructions") only mentions interventions. Jack learns: ACTION = interventions only.

**Root location:** `prompt.rs:198-206`.

### Prompt weakness: "Don't send the operator off" was added but wasn't enough
The three fixes from the prior round (prompt.rs:204, jack-chat.md:86-87,
jack-chat.md:132-133 + jack.md:135) addressed contradictions. But without fixing the
four code bugs above, Jack can try to act and fail silently.

---

## Changes Required

### Change 1: Return diagnostic Result from parse_action_from_response
**File:** `crates/russell-cli/src/commands/chat.rs`  
**Lines:** 588-626

Replace `Option<PendingAction>` with `Result<PendingAction, String>` where the error
string explains WHY parsing failed. The caller at line 530-554 prints the error when
parsing fails but an ACTION: line was present in the response.

Edge cases diagnosed:
- No ACTION line found (no message needed — Jack didn't propose one)
- Missing `/` separator
- Unknown skill ID
- Unknown action ID (neither probe nor intervention)
- Trailing text on ACTION line

### Change 2: Make consent gate require exact match
**File:** `crates/russell-cli/src/commands/chat.rs`  
**Lines:** 64-88, 237-266

- Keep `is_affirmative()` but it already only matches exact strings (no substring
  matching). The real fix is simpler: when a pending action is cleared by non-consent
  input, print a stronger message so the operator knows what happened.
- Add `"not now"`, `"later"`, `"hang on"`, `"wait"` to the denial list so these don't
  silently clear the action.
- After clearing on non-consent/non-denial input, pause briefly or print clearer text.

### Change 3: Raise max_tokens from 1024 to 2048
**File:** `crates/russell-cli/src/commands/chat.rs`  
**Line:** 938

Simple constant change. 2048 gives Jack room to explain then propose an action.

### Change 4: Teach ACTION for probes in objective text
**File:** `crates/russell-doctor/src/prompt.rs`  
**Lines:** 198-206

Add a probe example to the ACTION instruction. Show both:
```
ACTION: <skill-id>/<probe-id>  (e.g. ACTION: okapi-watcher/probe-health)
ACTION: <skill-id>/<intervention-id>  (e.g. ACTION: okapi-watcher/restart-okapi)
```
Clarify: probes auto-execute, interventions require consent.

---

## Files to Modify
1. `crates/russell-cli/src/commands/chat.rs` — parse_action, consent gate, max_tokens
2. `crates/russell-doctor/src/prompt.rs` — objective ACTION instruction

## Verification
- `cargo check` — must compile
- `cargo test -p russell-doctor -p russell-cli` — all tests must pass
- `cargo test` — full test suite
- Manual review: trace through a probe proposal in chat mode — user says "yes" after
  Jack proposes a probe → probe must execute without any "I'm just a watcher" refusal
