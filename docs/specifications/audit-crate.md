# /audit-crate

Audit a crate for tool/connector separation, parameterization, and
CTHA instrumentation readiness.

## Usage

```
/audit-crate <crate-path>
```

Example: `/audit-crate arsenal/crates/arsenal-pdf-knowledge`

## Instructions

Perform a three-layer architectural audit on the specified crate, applying
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

### Layer 3: CTHA Instrumentation (Cybernetic Health / Nervous System)

The insight: separating tools from connectors identifies the exact boundaries
where sensors belong. Every tool/connector boundary is a measurement point.

For each **TOOL** (adapter), identify what should be measured:
- **Input contract**: size, shape validity, expected range
- **Output contract**: did the transformation succeed? What was the output size?
- **Performance**: transformation duration (wall-clock)
- **Health signal**: ratio of successful transforms to total attempts

For each **CONNECTOR** (port), identify what should be measured:
- **Transfer health**: latency, throughput (bytes/sec or items/sec)
- **Reliability**: success/failure/retry count, error classification
- **Saturation**: queue depth, concurrent active transfers
- **Circuit state**: open/closed/half-open (if the connector has retry/circuit logic)

For the **orchestrator** (`main.rs` or equivalent), identify stage-level signals:
- **Stage progression**: which stage is active, how long each stage took
- **Pipeline throughput**: items processed per second at each stage
- **Bottleneck detection**: which stage is the slowest relative to input volume

#### Sensor placement rules:

1. **At tool boundaries**: instrument the function entry/exit with a tracing span
   that captures input size and output size. Use `ctha.tool.<module>.<function>` field prefix.
2. **At connector boundaries**: instrument with spans that capture latency,
   success/failure, and retry count. Use `ctha.connector.<module>.<target>` field prefix.
3. **At stage transitions**: emit an event when a pipeline stage completes with
   the stage duration, items processed, and success count.
   Use `ctha.pipeline.<stage_name>` field prefix.
4. **Error classification**: every error should carry a `ctha.error.class` field
   that categorizes it (timeout, parse_failure, model_refusal, io_error, validation_failure).

#### Naming convention for CTHA fields:

```
ctha.<layer>.<module>.<signal> = <value>

Layers: tool, connector, pipeline
Signals: duration_ms, items_in, items_out, success, error_class,
         latency_ms, retries, throughput_items_sec, circuit_state
```

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
- CTHA sensors needed: N

### Layer 1: Separation Issues

| File | Transformation (extract as tool) | Transfer (extract as connector) | Boundary |
|------|----------------------------------|--------------------------------|----------|
| ... | ... | ... | ... |

### Layer 2: Parameterization Issues

| File | Hardcoded value | Should be | Severity |
|------|----------------|-----------|----------|
| ... | ... | ... | high/medium/low |

### Layer 3: CTHA Instrumentation Plan

| Location | Type | Sensor | Fields | Priority |
|----------|------|--------|--------|----------|
| ocr_extract::extract_page_images | tool | span | ctha.tool.ocr_extract.duration_ms, items_out | high |
| ocr::transcribe_images | connector | span | ctha.connector.ocr.latency_ms, success, retries | high |
| ... | ... | ... | ... | ... |

### Recommended Refactoring Order

1. ...
2. ...
```

Severity/Priority guide:
- **high** — prevents reuse / blocks observability of critical path
- **medium** — requires source edit for variation / useful but not critical signal
- **low** — cosmetic / nice-to-have signal

### Principles Reference

- Tool = Adapter: transforms shape. Pure. Stateless.
- Connector = Port: transfers to/from boundary. Side effect. No logic.
- If a function does both, it is two operations that must be decomposed.
- The tool does not know where data goes. The connector does not know how data was formed.
- Either can be replaced, composed, or exposed through any surface independently.
- Every tool/connector boundary is a sensor placement point.
- CTHA fields use the prefix `ctha.<layer>.<module>.<signal>`.
- Error classification is mandatory — every failure carries `ctha.error.class`.
