# Scenario Tester Skill

**Version:** 1.0.0  
**Purpose:** Design, run, and evaluate test scenarios for Russell and agentic AI systems

## Overview

The `scenario-tester` skill provides automated testing for Russell's security features, probe functionality, and integration points. Tests are designed as executable bash scripts that can be run individually or as part of a full test suite.

## Available Probes

| Probe ID | Purpose | Duration |
|---|---|---|
| `probe-scenario-run-okapi` | Test Okapi integration scenarios | 180s |
| `probe-scenario-run-chat` | Test chat REPL scenarios | 120s |
| `probe-scenario-run-sentinel` | Test sentinel observation cycles | 30s |
| `probe-scenario-evaluate` | Evaluate test results | 30s |
| `probe-scenario-report` | Generate test reports | 15s |
| `probe-scenario-journal` | Verify journal integration | 10s |
| `probe-scenario-full` | Run complete test suite | 300s |
| `probe-scenario-test-capability-attenuation` | Test env filtering (Task S1) | 60s |
| `probe-scenario-test-prompt-sanitization` | Test sanitization (Task S3) | 60s |

## Running Tests

### Individual Test

```bash
# Run capability attenuation test
russell skill run scenario-tester/probe-scenario-test-capability-attenuation

# Run prompt sanitization test
russell skill run scenario-tester/probe-scenario-test-prompt-sanitization

# Run full test suite
russell skill run scenario-tester/probe-scenario-full
```

### Direct Script Execution

```bash
# Tests can also be run directly without Russell CLI
./skills/scenario-tester/scripts/scenario-test-capability-attenuation.sh
./skills/scenario-tester/scripts/scenario-test-prompt-sanitization.sh
```

## Test Descriptions

### Capability Attenuation Test

**Purpose:** Verify that skills only receive declared environment variables.

**What it tests:**
- Creates ephemeral test skill with no `allowed_env_keys`
- Sets `RUSSELL_*` variables in parent environment
- Verifies these variables are NOT leaked to skill subprocess
- Checks for other sensitive vars (API_KEY, SECRET, TOKEN, PASSWORD)

**Expected output:**
```
=== Capability Attenuation Scenario Test ===

Test setup:
  - Test skill directory: /tmp/tmp.XXXXXX
  - RUSSELL_* vars set in parent env

Running skill probe...
PASS: No RUSSELL_* or sensitive vars leaked to skill

✓ PASS: Capability attenuation working correctly
=== Test Complete ===
```

**Exit codes:**
- 0 — Capability attenuation working correctly
- 1 — Skill received undeclared environment variables
- 2 — Test setup failure

### Prompt Sanitization Test

**Purpose:** Verify that the `PromptSanitizer` correctly filters input/output.

**What it tests:**
1. **RUSSELL_* redaction** — Input containing env vars is redacted
2. **Shell metacharacter stripping** — `;|&$()` removed from input
3. **Prompt injection detection** — Phrases like "ignore previous" detected
4. **ACTION syntax validation** — Invalid skill/action IDs rejected
5. **Secret pattern redaction** — API keys, tokens redacted from output

**Expected output:**
```
=== Prompt Sanitization Scenario Test ===

Test 1: Input sanitization (RUSSELL_* redaction)
  ✓ PASS: RUSSELL_* redaction working
Test 2: Input sanitization (shell metacharacter stripping)
  ✓ PASS: Shell metacharacter stripping working
Test 3: Input sanitization (prompt injection detection)
  ✓ PASS: Prompt injection detection working
Test 4: Output sanitization (ACTION syntax validation)
  ✓ PASS: ACTION syntax validation working
Test 5: Output sanitization (secret pattern redaction)
  ✓ PASS: Secret pattern redaction working

=== Test Summary ===
Passed: 5 / 5
Failed: 0 / 5

✓ All prompt sanitization tests passed
```

**Exit codes:**
- 0 — All 5 tests passed
- 1 — One or more tests failed

## Adding New Scenarios

1. **Create script** in `scripts/scenario-test-<name>.sh`
2. **Follow pattern:**
   - Set up test environment (temp dirs, test data)
   - Run test scenario
   - Capture results
   - Clean up
   - Return appropriate exit code
3. **Add probe** to `manifest.yaml`:
   ```yaml
   probes:
     - id: probe-scenario-test-<name>
       cmd: ["bash", "./scripts/scenario-test-<name>.sh"]
       capture: stdout
       timeout: 60s
   ```
4. **Update symptoms** if testing new failure mode:
   ```yaml
   symptoms:
     - <new_symptom_name>
   ```

## Test Design Principles

1. **Idempotent** — Tests can be run multiple times safely
2. **Isolated** — Tests don't modify persistent state
3. **Fast** — Each test completes in <60 seconds
4. **Clear output** — Pass/fail status obvious from output
5. **Proper exit codes** — 0 = pass, non-zero = fail

## Troubleshooting

### Test Fails with "command not found"

Ensure `russell` binary is in PATH:
```bash
export PATH="$HOME/.local/bin:$PATH"
```

### Capability Attenuation Test Fails

Check that `russell` binary is installed and built with latest code:
```bash
cargo build --release
cp target/release/russell ~/.local/bin/
```

### Prompt Sanitization Test Fails

Run unit tests to verify sanitizer implementation:
```bash
cargo test --package russell-meta --lib sanitizer::tests
```

## References

- [ADR-0030](../../docs/adr/0030-prompt-sanitization-pipeline.md) — Prompt sanitization design
- [ADR-0031](../../docs/adr/0031-capability-attenuation.md) — Capability attenuation design
- [docs/USER_GUIDE.md](../../docs/USER_GUIDE.md) §4 — Security features overview
- [docs/standards/safety.md](../../docs/standards/safety.md) §9 — Runtime security features
