#!/usr/bin/env bash
# scenario-test-capability-attenuation.sh — verify skill env filtering
#
# Tests Task S1 (Capability Attenuation): skills only receive declared env vars.
# This probe verifies that the skill dispatcher correctly filters environment
# variables based on the manifest's allowed_env_keys declaration.
#
# Exit codes:
#   0 — Capability attenuation working correctly
#   1 — Skill received undeclared environment variables
#   2 — Test setup failure

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "=== Capability Attenuation Scenario Test ==="
echo ""

# Create a test skill with restricted env keys
TEST_SKDIR=$(mktemp -d)
TEST_MANIFEST="$TEST_SKDIR/manifest.yaml"
TEST_SCRIPT="$TEST_SKDIR/scripts/check-env.sh"

mkdir -p "$TEST_SKDIR/scripts"

# Create manifest with NO allowed_env_keys (should receive minimal env)
cat > "$TEST_MANIFEST" << 'EOF'
id: env-test-skill
version: 1.0.0
author: test
min_harness_version: 0.1.0
symptoms: []
applies_when:
  - os_family: linux
probes:
  - id: probe-check-env
    cmd: ["bash", "./scripts/check-env.sh"]
    capture: stdout
    timeout: 30s
interventions: []
EOF

# Create probe script that checks for RUSSELL_* env vars
cat > "$TEST_SCRIPT" << 'SCRIPT'
#!/usr/bin/env bash
# Check if any RUSSELL_* variables are present
# In capability attenuation mode, these should NOT be present

found_russell_vars=0
for var in $(env | grep "^RUSSELL_" || true); do
    echo "UNEXPECTED: Found $var"
    found_russell_vars=1
done

# Check for other sensitive vars that should be filtered
for var in $(env | grep -E "(API_KEY|SECRET|TOKEN|PASSWORD)" || true); do
    echo "UNEXPECTED: Found sensitive $var"
    found_russell_vars=1
done

if [ $found_russell_vars -eq 0 ]; then
    echo "PASS: No RUSSELL_* or sensitive vars leaked to skill"
    exit 0
else
    echo "FAIL: Skill received undeclared environment variables"
    exit 1
fi
SCRIPT

chmod +x "$TEST_SCRIPT"

# Set up test environment with RUSSELL_* variables
export RUSSELL_TEST_VAR="test_value_should_not_leak"
export RUSSELL_API_KEY="fake_api_key_should_not_leak"
export RUSSELL_LLM_URL="http://fake-url_should_not_leak"

echo "Test setup:"
echo "  - Test skill directory: $TEST_SKDIR"
echo "  - RUSSELL_* vars set in parent env"
echo ""

# Run the skill probe using russell skill run
echo "Running skill probe..."
if russell skill run --skill-dir "$TEST_SKDIR" env-test-skill probe-check-env 2>&1; then
    echo ""
    echo -e "${GREEN}✓ PASS: Capability attenuation working correctly${NC}"
    RESULT=0
else
    echo ""
    echo -e "${RED}✗ FAIL: Skill received undeclared environment variables${NC}"
    RESULT=1
fi

# Cleanup
rm -rf "$TEST_SKDIR"

echo ""
echo "=== Test Complete ==="
exit $RESULT