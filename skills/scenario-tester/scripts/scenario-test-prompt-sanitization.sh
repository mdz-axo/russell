#!/usr/bin/env bash
# scenario-test-prompt-sanitization.sh — verify input/output filtering
#
# Tests Task S3 (Prompt Sanitization Pipeline): verifies that the sanitizer
# correctly filters RUSSELL_* references from input and validates ACTION syntax.
#
# Exit codes:
#   0 — Prompt sanitization working correctly
#   1 — Sanitizer failed to filter sensitive data
#   2 — Test setup failure

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "=== Prompt Sanitization Scenario Test ==="
echo ""

# Test 1: Input sanitization (RUSSELL_* redaction)
echo "Test 1: Input sanitization (RUSSELL_* redaction)"
echo "  Testing that operator input containing RUSSELL_* is redacted..."

# Use cargo test to run the sanitizer unit tests
if cargo test --package russell-meta --lib sanitizer::tests::sanitize_input_redacts_russell_env --quiet 2>&1 | grep -q "test result: ok"; then
    echo -e "  ${GREEN}✓ PASS: RUSSELL_* redaction working${NC}"
    TEST1_PASS=true
else
    echo -e "  ${RED}✗ FAIL: RUSSELL_* redaction not working${NC}"
    TEST1_PASS=false
fi

# Test 2: Input sanitization (shell metacharacter stripping)
echo "Test 2: Input sanitization (shell metacharacter stripping)"
echo "  Testing that shell metacharacters are stripped from input..."

if cargo test --package russell-meta --lib sanitizer::tests::sanitize_input_strips_shell_metachars --quiet 2>&1 | grep -q "test result: ok"; then
    echo -e "  ${GREEN}✓ PASS: Shell metacharacter stripping working${NC}"
    TEST2_PASS=true
else
    echo -e "  ${RED}✗ FAIL: Shell metacharacter stripping not working${NC}"
    TEST2_PASS=false
fi

# Test 3: Input sanitization (injection detection)
echo "Test 3: Input sanitization (prompt injection detection)"
echo "  Testing that prompt injection phrases are detected..."

if cargo test --package russell-meta --lib sanitizer::tests::sanitize_input_detects_injection --quiet 2>&1 | grep -q "test result: ok"; then
    echo -e "  ${GREEN}✓ PASS: Prompt injection detection working${NC}"
    TEST3_PASS=true
else
    echo -e "  ${RED}✗ FAIL: Prompt injection detection not working${NC}"
    TEST3_PASS=false
fi

# Test 4: Output sanitization (ACTION validation)
echo "Test 4: Output sanitization (ACTION syntax validation)"
echo "  Testing that invalid ACTION: syntax is detected..."

if cargo test --package russell-meta --lib sanitizer::tests::sanitize_output_validates_action_syntax --quiet 2>&1 | grep -q "test result: ok"; then
    echo -e "  ${GREEN}✓ PASS: ACTION syntax validation working${NC}"
    TEST4_PASS=true
else
    echo -e "  ${RED}✗ FAIL: ACTION syntax validation not working${NC}"
    TEST4_PASS=false
fi

# Test 5: Output sanitization (secret redaction)
echo "Test 5: Output sanitization (secret pattern redaction)"
echo "  Testing that API keys/secrets are redacted from output..."

if cargo test --package russell-meta --lib sanitizer::tests::sanitize_output_redacts_secrets --quiet 2>&1 | grep -q "test result: ok"; then
    echo -e "  ${GREEN}✓ PASS: Secret pattern redaction working${NC}"
    TEST5_PASS=true
else
    echo -e "  ${RED}✗ FAIL: Secret pattern redaction not working${NC}"
    TEST5_PASS=false
fi

echo ""
echo "=== Test Summary ==="

PASS_COUNT=0
FAIL_COUNT=0

for test_result in "$TEST1_PASS" "$TEST2_PASS" "$TEST3_PASS" "$TEST4_PASS" "$TEST5_PASS"; do
    if [ "$test_result" = "true" ]; then
        ((PASS_COUNT++))
    else
        ((FAIL_COUNT++))
    fi
done

echo "Passed: $PASS_COUNT / 5"
echo "Failed: $FAIL_COUNT / 5"

if [ $FAIL_COUNT -eq 0 ]; then
    echo ""
    echo -e "${GREEN}✓ All prompt sanitization tests passed${NC}"
    exit 0
else
    echo ""
    echo -e "${RED}✗ Some prompt sanitization tests failed${NC}"
    exit 1
fi