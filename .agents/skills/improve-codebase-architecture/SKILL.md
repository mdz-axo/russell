---
name: improve-codebase-architecture
visibility: public
description: "Find deepening opportunities in a codebase. Use when the user wants to improve architecture, find refactoring opportunities, consolidate tightly-coupled modules, make a codebase more testable, or says 'this is a ball of mud'."
---

# Improve Codebase Architecture

Surface architectural friction and propose **deepening opportunities** — refactors that turn shallow modules into deep ones. The aim is testability and navigability.

Adapted from Matt Pocock's skill.

## Glossary

Use these terms exactly. Consistent language is the point.

- **Module** — anything with an interface and an implementation (function, class, package, crate).
- **Interface** — everything a caller must know to use the module: types, invariants, error modes, ordering, config. Not just the type signature.
- **Implementation** — the code inside.
- **Depth** — leverage at the interface: a lot of behaviour behind a small interface. **Deep** = high leverage. **Shallow** = interface nearly as complex as the implementation.
- **Seam** — where an interface lives; a place behaviour can be altered without editing in place.
- **Adapter** — a concrete thing satisfying an interface at a seam.
- **Leverage** — what callers get from depth.
- **Locality** — what maintainers get from depth: change, bugs, knowledge concentrated in one place.

Key principles:

- **Deletion test**: imagine deleting the module. If complexity vanishes, it was a pass-through. If complexity reappears across N callers, it was earning its keep.
- **The interface is the test surface.**
- **One adapter = hypothetical seam. Two adapters = real seam.**

## Process

### 1. Explore

Read the project's domain glossary and any ADRs first. Then walk the codebase organically and note where you experience friction:

- Where does understanding one concept require bouncing between many small modules?
- Where are modules **shallow** — interface nearly as complex as the implementation?
- Where have pure functions been extracted just for testability, but the real bugs hide in how they're called (no **locality**)?
- Where do tightly-coupled modules leak across their seams?
- Which parts of the codebase are untested, or hard to test through their current interface?

Apply the **deletion test** to anything you suspect is shallow: would deleting it concentrate complexity, or just move it?

### 2. Present candidates

For each candidate:

- **Files** — which files/modules are involved
- **Problem** — why the current architecture is causing friction
- **Solution** — plain English description of what would change
- **Benefits** — explained in terms of locality and leverage, and how tests would improve
- **Recommendation strength** — one of `Strong`, `Worth exploring`, `Speculative`

Use the project's domain vocabulary. Do NOT propose interfaces yet. Ask: "Which of these would you like to explore?"

### 3. Grilling loop

Once the user picks a candidate, walk the design tree with them — constraints, dependencies, the shape of the deepened module, what sits behind the seam, what tests survive.

Side effects happen inline as decisions crystallize:

- **Naming a deepened module after a concept not in the glossary?** Add the term.
- **Sharpening a fuzzy term?** Update the glossary right there.
- **User rejects the candidate with a load-bearing reason?** Offer to record it as an ADR. Only offer when the reason would actually be needed by a future explorer.