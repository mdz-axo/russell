---
name: zoom-out
visibility: public
description: Zoom out to give broader context or a higher-level perspective on unfamiliar code. Use when you're unfamiliar with a section of code, need to understand how it fits into the bigger picture, or when you're lost in the weeds.
---

# Zoom Out

Go up a layer of abstraction. Provide a map of all relevant modules and callers, using the project's domain glossary vocabulary.

## What to Produce

1. **Module map** — List the relevant modules and what each one owns, using domain terms from the project's glossary or architecture docs.

2. **Caller graph** — Show who calls what, traced through public interfaces (seams), not internal function chains. Use domain vocabulary: "Dispatcher sends events to Handler" not "`dispatcher.rs:L142` calls `handler::process()`".

3. **Data flow** — How the key data flows through the system at the current level of abstraction.

4. **Boundary summary** — Where the current code sits relative to module boundaries. What crosses those boundaries and how.

5. **Key invariants** — Any important constraints that aren't obvious from the code.

## Constraints

- Use the project's own domain language, not generic terms.
- When uncertain about a module's purpose, say so explicitly rather than guessing.
- Focus on the current module and its immediate neighbors — don't map the entire system unless asked.