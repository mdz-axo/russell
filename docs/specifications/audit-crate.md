# /audit-crate

Audit a crate for tool/connector separation and parameterization correctness.

## Usage

```
/audit-crate <crate-path>
```

Example: `/audit-crate arsenal/crates/arsenal-pdf-knowledge`

## Instructions

Perform a two-layer architectural audit on the specified crate, applying
the Kask platform's core discipline (see `docs/architecture/ARCHITECTURE_DEFINITION.md` §1.1).

### Layer 1: Tool/Connector Separation

For every `.rs` file in the crate's `src/` directory:

1. Read the module doc comment and key function signatures.
2. Classify the file as one of:
   - **TOOL** (adapter) — transforms data from one shape to another. Pure. No side effects. No I/O.
   - **CONNECTOR** (port) — transfers formed data to/from a boundary. Side effect. No transformation logic.
   - **CONFLATED** — does both transformation and transfer in the same code path.
3. For each CONFLATED file, identify:
   - What is the transformation? (the tool that should be extracted)
   - What is the transfer? (the connector that should be extracted)
   - Where is the boundary between them? (the point where formed data is handed off)

### Layer 2: Parameterization

For every file (including already-clean tools and connectors):

1. Identify values that are hardcoded but should be parameters:
   - Constants that vary by use case (timeouts, thresholds, model IDs, prompt text)
   - Decisions baked into function bodies that should flow in from callers
   - Configuration that would require source edits to change for a different context
2. The audit question: **"If I wanted to use this tool/connector in a different context, what would I have to edit in the source code vs what could I pass as an argument?"**
3. Anything requiring a source edit is a parameterization defect.

Exceptions (not defects):
- Truly universal constants (mathematical, protocol-defined)
- Type system constraints (enum variants that define the domain model)
- Validation rules that ARE the tool's purpose

### Output Format

Produce a structured report:

```markdown
## Crate Audit: <crate-name>

### Summary
- Files audited: N
- Clean (tool): N
- Clean (connector): N
- Conflated: N
- Parameterization issues: N

### Layer 1: Separation Issues

| File | Transformation (extract as tool) | Transfer (extract as connector) | Boundary |
|------|----------------------------------|--------------------------------|----------|
| ... | ... | ... | ... |

### Layer 2: Parameterization Issues

| File | Hardcoded value | Should be | Severity |
|------|----------------|-----------|----------|
| ... | ... | ... | high/medium/low |

### Recommended Refactoring Order

1. ...
2. ...
```

Severity guide:
- **high** — prevents reuse in a different context entirely
- **medium** — requires source edit for reasonable variation
- **low** — cosmetic or unlikely to vary in practice

### Principles Reference

- Tool = Adapter: transforms shape. Pure. Stateless.
- Connector = Port: transfers to/from boundary. Side effect. No logic.
- If a function does both, it is two operations that must be decomposed.
- The tool does not know where data goes. The connector does not know how data was formed.
- Either can be replaced, composed, or exposed through any surface independently.
