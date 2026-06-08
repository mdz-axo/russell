---
title: "ADR-0053: Generative Settings Exposure in Chat REPL"
audience: [developers, architects, operators]
last_updated: 2026-06-07
ddmvss_context: "cross-cutting"
ddmvss_artifact: "adr"
version: "1.0.0"
status: "Proposed"
domain: "Cross-cutting"
---

# ADR-0053: Generative Settings Exposure in Chat REPL

## Status

**Proposed**

## Context

Magna Carta P3 (Generative Space) requires that all generative settings be exposed to operators. The Magna Carta §Settings Exposure states:

> Inference and tooling must expose all probabilistic/generative settings to operators — temperature, top-k, top-p, repeat penalty, and any other parameters the underlying model or tool supports. No settings are hidden or admin-gated.

Currently, Russell's chat REPL (`russell chat`) provides no mechanism for the operator to view or modify LLM inference settings. The settings — temperature, top-k, top-p, repeat penalty, and the HHH (Helpful, Harmless, Honest) filter state — are configured in the `Profile` struct but are not accessible during a chat session. The operator cannot:

- See what inference settings are active.
- Adjust temperature or sampling parameters mid-session.
- Disable the HHH filter (the Magna Carta §Operator Curation, Not System Imposition states: "Disabling HHH mode is possible and produces unfiltered output at the declared temperature").
- Switch personas without restarting the session.

This violates P3: settings are de facto hidden from the operator, and the principle that "if an internal engineer can adjust a parameter, the operator can too" is not met.

## Decision

Add a `/settings` command and two CLI flags to the chat REPL:

1. **`/settings` REPL command** — Displays and modifies generative settings in-session:
   - `/settings` — Show current settings (temperature, top-k, top-p, repeat penalty, HHH filter state, active persona).
   - `/settings temperature 0.8` — Set temperature.
   - `/settings top_k 40` — Set top-k.
   - `/settings top_p 0.95` — Set top-p.
   - `/settings repeat_penalty 1.1` — Set repeat penalty.
   - `/settings hhh on|off` — Enable or disable HHH filter.
   - `/settings reset` — Revert all settings to profile defaults.

2. **`--no-hhh` CLI flag** — Starts the chat session with HHH filter disabled. Equivalent to `/settings hhh off` at session start.

3. **`--persona <name>` CLI flag** — Starts the chat session with the named persona. Equivalent to switching persona at session start.

4. **Persistence** — Settings changed via `/settings` persist across sessions through `russell_core::profile::Profile`. The profile is written on change and loaded on session start. The `--no-hhh` and `--persona` flags override the profile for the duration of the session and are persisted as the new defaults (matching the Magna Carta's principle that operator curation replaces system defaults).

## Consequences

### Positive

- **P3 compliance** — All generative settings are visible and adjustable by the operator.
- **Operator curation** — The operator can disable HHH, change temperature, and switch persona — the system does not impose defaults.
- **No privileged access** — The same settings surface available to engineers is available to operators.
- **Session continuity** — Settings persist via `Profile`, so the operator's choices survive session restarts.
- **Discoverability** — `/settings` with no arguments shows the current state, making the settings surface self-documenting.

### Negative

- **Profile writes on every change** — Each `/settings` invocation writes the profile to disk. Frequent adjustments could cause excessive I/O. Mitigation: profile writes are small (< 1 KiB) and occur at most once per REPL command.
- **Persona proliferation** — Operators may create many persona configurations. There is no built-in mechanism to list or delete personas from the REPL. Mitigation: persona management can be added as a future `/persona` command.
- **HHH-off risk** — Disabling the HHH filter may produce outputs that are harmful or offensive. The Magna Carta §Operator Curation explicitly acknowledges this: "Disabling HHH mode is possible and produces unfiltered output at the declared temperature." The operator consents to this by running `--no-hhh` or `/settings hhh off`.

### Risks

- **Unsafe settings** — An operator could set temperature to 2.0 or top-k to 1, producing degenerate output. Mitigation: `/settings` displays a warning when values are outside recommended ranges, but does not block the change (P3: no system-imposed constraints).
- **Profile corruption** — If the profile file is corrupted (disk full, crash mid-write), settings may be lost or invalid. Mitigation: profile writes are atomic (write-to-temp, rename); the `Profile::load()` method falls back to defaults on parse errors.
- **Persona not found** — `--persona <name>` with an unknown name should fail gracefully with a clear error, not silently fall back to the default persona.

## Implementation

### Code Changes

| File | Change |
|---|---|
| `crates/russell-cli/src/commands/chat/mod.rs` | Parse `/settings` command; dispatch to `SettingsHandler`; add `--no-hhh` and `--persona` CLI flags |
| `crates/russell-cli/src/commands/chat/settings.rs` (new) | `SettingsHandler`: display current settings, apply changes, persist via `Profile` |
| `crates/russell-core/src/profile.rs` | Add `InferenceSettings` struct with `temperature`, `top_k`, `top_p`, `repeat_penalty`, `hhh_enabled`, `persona`; add load/save methods |
| `crates/russell-cli/src/commands/chat/mod.rs` | Apply `--no-hhh` and `--persona` flags at session start, overriding loaded profile |

### InferenceSettings Struct

```rust
pub struct InferenceSettings {
    pub temperature: f32,
    pub top_k: u32,
    pub top_p: f32,
    pub repeat_penalty: f32,
    pub hhh_enabled: bool,
    pub persona: String,
}
```

### `/settings` Command Flow

1. Operator types `/settings` or `/settings <key> <value>`.
2. REPL parser matches the `/settings` prefix.
3. `SettingsHandler::handle(args)` is called.
   - No args: display current `InferenceSettings`.
   - Key-value args: validate, apply to session state, persist via `Profile::save()`.
4. On next LLM invocation, the session uses the updated settings.

### CLI Flag Integration

```
russell chat [--no-hhh] [--persona <name>]
```

- `--no-hhh`: Set `hhh_enabled = false` in session, persist to profile.
- `--persona <name>`: Load named persona, set as active, persist to profile.

### Testing Strategy

- Unit tests for `SettingsHandler`: parse valid/invalid key-value pairs.
- Unit tests for `InferenceSettings` validation: warn on out-of-range values, accept any value.
- Integration test: `/settings temperature 0.8` changes the LLM invocation parameters in the next turn.
- Integration test: `--no-hhh` flag disables HHH filter and persists to profile.
- Integration test: `/settings reset` reverts to profile defaults.

## References

- [Magna Carta §P3: Generative Space](../architecture/magna-carta.md#principle-3-generative-space)
- [Magna Carta §Settings Exposure](../architecture/magna-carta.md#settings-exposure)
- [Magna Carta §Operator Curation, Not System Imposition](../architecture/magna-carta.md#operator-curation-not-system-imposition)
- [Magna Carta §No Privileged Engineer Access](../architecture/magna-carta.md#no-privileged-engineer-access)
- [ADR-0049: Three-Surface Interaction](0049-three-surface-interaction.md) — chat REPL as primary surface
- [AGENTS.md](../../AGENTS.md) — vocabulary: Chat REPL, Generative Space, Non-Normativity

## Appendix

### Alternatives Considered

#### Alternative 1: Environment variable overrides

**Description:** Allow operators to override inference settings via environment variables (`RUSSELL_TEMPERATURE=0.8`, `RUSSELL_NO_HHH=1`).

**Pros:**
- No REPL changes needed
- Works with scripting and automation

**Cons:**
- Not discoverable — the operator must know the variable names
- Not adjustable mid-session — environment variables are read at startup
- Violates P3's requirement that settings be *exposed*, not hidden behind env vars
- Mixing env vars with profile settings creates precedence ambiguity

**Why rejected:** P3 requires settings to be exposed in the interface the operator uses (the chat REPL), not hidden in environment variables that require external documentation.

#### Alternative 2: Separate `/set` command per parameter

**Description:** Individual commands like `/temperature 0.8`, `/top_k 40`, `/no-hhh`.

**Pros:**
- Shorter command syntax for common adjustments
- No nested parsing

**Cons:**
- Command namespace pollution — each setting claims a top-level `/` command
- No unified display command (`/settings` with no args shows everything)
- Harder to add new settings without adding new `/` commands
- Inconsistent with REPL conventions (most chat REPLs use `/set` or `/settings`)

**Why rejected:** A single `/settings` command with key-value arguments is more scalable and discoverable. Individual commands would clutter the `/` namespace.

#### Alternative 3: Edit profile file directly

**Description:** Document the profile path and tell operators to edit the YAML/TOML file.

**Pros:**
- Zero implementation cost
- Full control over all profile fields

**Cons:**
- Requires leaving the chat session to change settings
- No validation — invalid values cause silent failures or crashes
- Not accessible from ACP or other surfaces
- Violates P3: the operator must be able to adjust settings *within the generative space*, not by leaving it

**Why rejected:** The Magna Carta §Settings Exposure requires that inference settings be exposed *to operators* in the interface they use. Direct file editing is not exposure — it is an implementation detail.