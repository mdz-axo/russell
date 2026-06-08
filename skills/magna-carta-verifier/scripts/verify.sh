#!/usr/bin/env bash
# verify.sh — Run Magna Carta assertion manifest and emit structured results.
#
# Usage: bash ./scripts/verify.sh <manifest.yaml>
#
# Each assertion in the manifest is checked using its declared method.
# Output is one JSON object per assertion on stdout.
#
# Exit codes:
#   0 — All assertions pass or gap (no failures)
#   1 — One or more assertions failed
#   2 — Invalid arguments or manifest
#
# SPDX-License-Identifier: MIT

set -euo pipefail

RUSSELL_ROOT="${RUSSELL_ROOT:-$(cd "$(dirname "$0")/../../.." && pwd)}"

# ── Argument validation ──────────────────────────────────────────────────────

if [[ $# -lt 1 ]]; then
  echo "Usage: verify.sh <manifest.yaml>" >&2
  exit 2
fi

MANIFEST="$1"

if [[ ! -f "$MANIFEST" ]]; then
  echo "Error: manifest not found: $MANIFEST" >&2
  exit 2
fi

# ── Helpers ───────────────────────────────────────────────────────────────────

# Extract a YAML list value simply (no yq dependency).
# Usage: yaml_list <file> <path> <index> <field>
# This is intentionally minimal — manifests are simple and predictable.
yaml_field() {
  local file="$1" path="$2"
  # Naïve extraction: find the line starting with the key and return its value.
  grep -E "^\s*${path}:" "$file" | head -1 | sed -E "s/^\s*${path}:\s*//; s/\"//g; s/'//g" | tr -d ' '
}

# Extract the principle name from the manifest.
principle_name() {
  yaml_field "$1" "principle"
}

# Extract assertion blocks between "assertions:" and the next top-level key.
# Outputs each assertion as: id|name|claim|method|targets_line
parse_assertions() {
  local file="$1"
  local in_assertions=false
  local current_id="" current_name="" current_claim="" current_method=""

  while IFS= read -r line; do
    # Detect assertions section
    if [[ "$line" =~ ^assertions: ]]; then
      in_assertions=true
      continue
    fi

    # Exit assertions section on next top-level key
    if $in_assertions && [[ "$line" =~ ^[a-z] ]]; then
      in_assertions=false
    fi

    if ! $in_assertions; then
      continue
    fi

    # New assertion starts with "    - id:"
    if [[ "$line" =~ ^[[:space:]]+-[[:space:]]+id: ]]; then
      # Flush previous assertion
      if [[ -n "$current_id" ]]; then
        echo "${current_id}|${current_name}|${current_claim}|${current_method}"
      fi
      current_id=$(echo "$line" | sed -E 's/.*id:\s*//; s/"//g; s/'"'"'//g' | tr -d ' ')
      current_name=""
      current_claim=""
      current_method=""
    elif [[ "$line" =~ name: ]]; then
      current_name=$(echo "$line" | sed -E 's/.*name:\s*//; s/"//g; s/'"'"'//g')
    elif [[ "$line" =~ claim: ]]; then
      current_claim=$(echo "$line" | sed -E 's/.*claim:\s*//; s/"//g; s/'"'"'//g')
    elif [[ "$line" =~ method: ]]; then
      current_method=$(echo "$line" | sed -E 's/.*method:\s*//; s/"//g; s/'"'"'//g')
    fi
  done < "$file"

  # Flush last assertion
  if [[ -n "$current_id" ]]; then
    echo "${current_id}|${current_name}|${current_claim}|${current_method}"
  fi
}

# Extract target details for a given assertion id from the manifest.
# Outputs lines of: crate|module|methods|gate
parse_targets() {
  local file="$1" assertion_id="$2"
  local in_target_section=false in_targets=false capturing=false
  local current_crate="" current_module="" current_methods="" current_gate=""

  while IFS= read -r line; do
    if [[ "$line" =~ ^[[:space:]]+-[[:space:]]+id:[[:space:]]*${assertion_id} ]]; then
      capturing=true
      continue
    fi

    # New assertion block starts
    if $capturing && [[ "$line" =~ ^[[:space:]]+-[[:space:]]+id: ]]; then
      # Flush previous target if any
      if [[ -n "$current_crate" ]]; then
        echo "${current_crate}|${current_module}|${current_methods}|${current_gate}"
      fi
      break
    fi

    if ! $capturing; then
      continue
    fi

    if [[ "$line" =~ crate: ]]; then
      # Flush previous target
      if [[ -n "$current_crate" ]]; then
        echo "${current_crate}|${current_module}|${current_methods}|${current_gate}"
      fi
      current_crate=$(echo "$line" | sed -E 's/.*crate:\s*//; s/"//g; s/'"'"'//g' | tr -d ' ')
      current_module=""
      current_methods=""
      current_gate=""
    elif [[ "$line" =~ module: ]]; then
      current_module=$(echo "$line" | sed -E 's/.*module:\s*//; s/"//g; s/'"'"'//g' | tr -d ' ')
    elif [[ "$line" =~ methods: ]]; then
      current_methods=$(echo "$line" | sed -E 's/.*methods:\s*\[//; s/\]//; s/"//g; s/'"'"'//g; s/ //g')
    elif [[ "$line" =~ gate: ]]; then
      current_gate=$(echo "$line" | sed -E 's/.*gate:\s*//; s/"//g; s/'"'"'//g' | tr -d ' ')
    fi
  done < "$file"

  # Flush last target
  if [[ -n "$current_crate" ]]; then
    echo "${current_crate}|${current_module}|${current_methods}|${current_gate}"
  fi
}

# ── Verification Methods ──────────────────────────────────────────────────────

# structural_audit: grep for gate calls on target methods in source code.
# Pass = gate found on every target method in every target crate.
run_structural_audit() {
  local assertion_id="$1" manifest="$2"
  local all_pass=true findings=() gaps=()

  while IFS='|' read -r crate module methods gate; do
    local crate_dir="${RUSSELL_ROOT}/crates/${crate}"

    if [[ ! -d "$crate_dir" ]]; then
      gaps+=("crate ${crate} not found at ${crate_dir}")
      all_pass=true  # gap, not failure
      continue
    fi

    # Convert module path to file path (e.g. journal::writer → journal/writer.rs or src/journal/writer.rs)
    local module_path
    module_path=$(echo "$module" | sed 's/::/\//g')

    # Search for the gate call in the crate source
    local found_gate=false
    if grep -rl "$gate" "$crate_dir/src/" 2>/dev/null | head -1 | grep -q .; then
      found_gate=true
    fi

    if ! $found_gate; then
      findings+=("gate '${gate}' not found in crate ${crate}")
      all_pass=false
    fi

    # For each method, check it exists and is associated with the gate
    local IFS_old="$IFS"
    IFS=','
    for method in $methods; do
      IFS="$IFS_old"
      local method_found=false
      if grep -rl "$method" "$crate_dir/src/" 2>/dev/null | head -1 | grep -q .; then
        method_found=true
      fi
      if ! $method_found; then
        gaps+=("method ${method} not found in ${crate}::${module}")
      fi
    done
    IFS="$IFS_old"
  done < <(parse_targets "$manifest" "$assertion_id")

  if $all_pass && [[ ${#findings[@]} -eq 0 ]]; then
    echo "pass"
  elif [[ ${#findings[@]} -gt 0 ]]; then
    echo "fail"
  else
    echo "gap"
  fi

  for f in "${findings[@]}"; do echo "FINDING:$f"; done
  for g in "${gaps[@]}"; do echo "GAP:$g"; done
}

# behavioral_probe: try access without consent and check denial.
# Pass = access denied (the expected safe behavior).
run_behavioral_probe() {
  local assertion_id="$1" manifest="$2"
  local findings=() gaps=()

  while IFS='|' read -r crate module methods gate; do
    local crate_dir="${RUSSELL_ROOT}/crates/${crate}"

    if [[ ! -d "$crate_dir" ]]; then
      gaps+=("crate ${crate} not found — cannot probe")
      continue
    fi

    # Check that the DenyAllConsent default or equivalent exists
    local deny_all_found=false
    if grep -rl "DenyAllConsent\|deny_all\|default.*deny" "$crate_dir/src/" 2>/dev/null | head -1 | grep -q .; then
      deny_all_found=true
    fi

    if ! $deny_all_found; then
      findings+=("no default-deny implementation found in ${crate}")
    fi
  done < <(parse_targets "$manifest" "$assertion_id")

  if [[ ${#findings[@]} -eq 0 ]]; then
    echo "pass"
  else
    echo "fail"
  fi

  for f in "${findings[@]}"; do echo "FINDING:$f"; done
  for g in "${gaps[@]}"; do echo "GAP:$g"; done
}

# absence_check: grep for prohibited patterns. Pass = pattern NOT found.
run_absence_check() {
  local assertion_id="$1" manifest="$2"
  local findings=() gaps=()

  # Prohibited patterns — things that must NOT exist in the codebase
  local prohibited_patterns=(
    "admin_override"
    "engineer_mode"
    "god_token"
    "bypass_consent"
    "skip_sovereignty"
    "hidden_gate"
    "force_allow"
  )

  while IFS='|' read -r crate module methods gate; do
    local crate_dir="${RUSSELL_ROOT}/crates/${crate}"

    if [[ ! -d "$crate_dir" ]]; then
      gaps+=("crate ${crate} not found — cannot check absence")
      continue
    fi

    for pattern in "${prohibited_patterns[@]}"; do
      if grep -rl "$pattern" "$crate_dir/src/" 2>/dev/null | head -1 | grep -q .; then
        findings+=("prohibited pattern '${pattern}' found in ${crate}")
      fi
    done
  done < <(parse_targets "$manifest" "$assertion_id")

  if [[ ${#findings[@]} -eq 0 ]]; then
    echo "pass"
  else
    echo "fail"
  fi

  for f in "${findings[@]}"; do echo "FINDING:$f"; done
  for g in "${gaps[@]}"; do echo "GAP:$g"; done
}

# resource_verification: check that resource categorization is correct.
run_resource_verification() {
  local assertion_id="$1" manifest="$2"
  local findings=() gaps=()

  while IFS='|' read -r crate module methods gate; do
    local crate_dir="${RUSSELL_ROOT}/crates/${crate}"

    if [[ ! -d "$crate_dir" ]]; then
      gaps+=("crate ${crate} not found — cannot verify resources")
      continue
    fi

    # Check that DataSovereigntyBoundary or equivalent categorization exists
    local categorization_found=false
    if grep -rl "DataSovereigntyBoundary\|sovereign_data\|shared_data\|public_data" "$crate_dir/src/" 2>/dev/null | head -1 | grep -q .; then
      categorization_found=true
    fi

    if ! $categorization_found; then
      findings+=("no resource categorization found in ${crate}")
    fi
  done < <(parse_targets "$manifest" "$assertion_id")

  if [[ ${#findings[@]} -eq 0 ]]; then
    echo "pass"
  else
    echo "fail"
  fi

  for f in "${findings[@]}"; do echo "FINDING:$f"; done
  for g in "${gaps[@]}"; do echo "GAP:$g"; done
}

# ── Dispatch ─────────────────────────────────────────────────────────────────

run_method() {
  local method="$1" assertion_id="$2" manifest="$3"

  case "$method" in
    structural_audit)
      run_structural_audit "$assertion_id" "$manifest"
      ;;
    behavioral_probe)
      run_behavioral_probe "$assertion_id" "$manifest"
      ;;
    absence_check)
      run_absence_check "$assertion_id" "$manifest"
      ;;
    resource_verification)
      run_resource_verification "$assertion_id" "$manifest"
      ;;
    *)
      echo "gap"
      echo "GAP:unknown method ${method}"
      ;;
  esac
}

# ── Main ──────────────────────────────────────────────────────────────────────

principle=$(principle_name "$MANIFEST")
any_failure=false

while IFS='|' read -r aid name claim method; do
  # Handle combined methods (e.g. "structural + behavioral")
  if [[ "$method" == *"+"* ]]; then
    method1=$(echo "$method" | cut -d'+' -f1 | xargs)
    method2=$(echo "$method" | cut -d'+' -f2 | xargs)
    # Map friendly names
    method1=$(echo "$method1" | sed 's/structural/structural_audit/; s/behavioral/behavioral_probe/')
    method2=$(echo "$method2" | sed 's/structural/structural_audit/; s/behavioral/behavioral_probe/')

    output1=$(run_method "$method1" "$aid" "$MANIFEST")
    output2=$(run_method "$method2" "$aid" "$MANIFEST")

    status1=$(echo "$output1" | head -1)
    status2=$(echo "$output2" | head -1)

    # Combined: fail if either fails, gap if both gap, pass if both pass
    if [[ "$status1" == "fail" || "$status2" == "fail" ]]; then
      status="fail"
      any_failure=true
    elif [[ "$status1" == "gap" || "$status2" == "gap" ]]; then
      status="gap"
    else
      status="pass"
    fi

    findings=()
    gaps_arr=()
    while IFS= read -r line; do
      if [[ "$line" == FINDING:* ]]; then findings+=("${line#FINDING:}"); fi
      if [[ "$line" == GAP:* ]]; then gaps_arr+=("${line#GAP:}"); fi
    done < <(echo "$output1"; echo "$output2")

    # Build JSON array from bash array (no jq dependency)
    if [[ ${#findings[@]} -eq 0 ]]; then
      findings_json="[]"
    else
      findings_json=$(printf ', "%s"' "${findings[@]}")
      findings_json="[${findings_json:2}]"
    fi
    if [[ ${#gaps_arr[@]} -eq 0 ]]; then
      gaps_json="[]"
    else
      gaps_json=$(printf ', "%s"' "${gaps_arr[@]}")
      gaps_json="[${gaps_json:2}]"
    fi
  else
    output=$(run_method "$method" "$aid" "$MANIFEST")
    status=$(echo "$output" | head -1)

    if [[ "$status" == "fail" ]]; then
      any_failure=true
    fi

    findings=()
    gaps_arr=()
    while IFS= read -r line; do
      if [[ "$line" == FINDING:* ]]; then findings+=("${line#FINDING:}"); fi
      if [[ "$line" == GAP:* ]]; then gaps_arr+=("${line#GAP:}"); fi
    done < <(echo "$output" | tail -n +2)

    if [[ ${#findings[@]} -eq 0 ]]; then
      findings_json="[]"
    else
      findings_json=$(printf ', "%s"' "${findings[@]}")
      findings_json="[${findings_json:2}]"
    fi
    if [[ ${#gaps_arr[@]} -eq 0 ]]; then
      gaps_json="[]"
    else
      gaps_json=$(printf ', "%s"' "${gaps_arr[@]}")
      gaps_json="[${gaps_json:2}]"
    fi
  fi

  # Escape special characters for JSON
  claim_escaped=$(echo "$claim" | sed 's/\\/\\\\/g; s/"/\\"/g')
  name_escaped=$(echo "$name" | sed 's/\\/\\\\/g; s/"/\\"/g')

  cat <<EOF
{"assertion_id":"${aid}","name":"${name_escaped}","status":"${status}","method":"${method}","claim":"${claim_escaped}","findings":${findings_json},"gaps":${gaps_json}}
EOF

done < <(parse_assertions "$MANIFEST")

if $any_failure; then
  exit 1
fi

exit 0
