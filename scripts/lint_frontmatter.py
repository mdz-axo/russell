#!/usr/bin/env python3
"""CI lint: validate TOGAF-Lite frontmatter schema on markdown files.

Usage:
    python3 scripts/lint_frontmatter.py [path ...]

Exit codes:
    0 — all files pass
    1 — validation failures detected

Validates:
    1. YAML frontmatter block (--- ... ---)
    2. Required fields: title, audience, last_updated, togaf_phase, version, status
    3. HTML comment block: TOGAF_DOMAIN, VERSION, STATUS, LAST_UPDATED
    4. togaf_phase values: Preliminary, A, B, C, D, E, F, G, H, Requirements Management
    5. status values: Active, Proposed, Superseded, Deprecated, Draft
"""

import os
import re
import sys
import yaml

REQUIRED_FIELDS = ["title", "audience", "last_updated", "togaf_phase", "version", "status"]
VALID_PHASES = [
    "Preliminary", "A", "B", "C", "D", "E", "F", "G", "H", "Requirements Management"
]
VALID_STATUSES = ["Active", "Proposed", "Superseded", "Deprecated", "Draft"]
SKIP_DIRS = {"archive", "generated", "evolution-digest"}

def validate_file(filepath):
    """Validate a single markdown file. Returns list of error strings."""
    errors = []
    
    with open(filepath, "r") as f:
        content = f.read()
    
    # 1. Check YAML frontmatter
    fm_match = re.match(r'^---\n(.*?)\n---', content, re.DOTALL)
    if not fm_match:
        errors.append("Missing YAML frontmatter block (--- ... ---)")
        return errors
    
    fm_text = fm_match.group(1)
    try:
        fm = yaml.safe_load(fm_text)
    except yaml.YAMLError as e:
        errors.append(f"YAML parse error: {e}")
        return errors
    
    if not isinstance(fm, dict):
        errors.append("Frontmatter is not a YAML mapping")
        return errors
    
    # 2. Check required fields
    for field in REQUIRED_FIELDS:
        if field not in fm:
            errors.append(f"Missing required field: {field}")
    
    # 3. Validate togaf_phase
    phase = fm.get("togaf_phase", "")
    if phase and phase not in VALID_PHASES:
        errors.append(f"Invalid togaf_phase: '{phase}' (must be one of: {', '.join(VALID_PHASES)})")
    
    # 4. Validate status
    status = fm.get("status", "")
    if status and status not in VALID_STATUSES:
        errors.append(f"Invalid status: '{status}' (must be one of: {', '.join(VALID_STATUSES)})")
    
    # 5. Check HTML comment block
    html_comments = re.findall(r'<!--\s*(TOGAF_DOMAIN|VERSION|STATUS|LAST_UPDATED):\s*(.+?)\s*-->', content)
    found_keys = {k for k, v in html_comments}
    required_keys = {"TOGAF_DOMAIN", "VERSION", "STATUS", "LAST_UPDATED"}
    missing_keys = required_keys - found_keys
    if missing_keys:
        errors.append(f"Missing HTML comment tags: {', '.join(sorted(missing_keys))}")
    
    # 6. Check mermaid diagrams have DIAGRAM_ALIGNMENT
    mermaid_count = len(re.findall(r'```mermaid', content))
    alignment_count = len(re.findall(r'DIAGRAM_ALIGNMENT', content))
    if mermaid_count > 0 and alignment_count == 0:
        errors.append(f"Has {mermaid_count} mermaid block(s) but no DIAGRAM_ALIGNMENT metadata")
    
    return errors

def main():
    paths = sys.argv[1:]
    if not paths:
        # Default: scan current directory
        paths = [os.getcwd()]
    
    total = 0
    passed = 0
    failed = 0
    all_errors = []
    
    for path in paths:
        if os.path.isfile(path) and path.endswith(".md"):
            files = [path]
        elif os.path.isdir(path):
            files = []
            for root, dirs, filenames in os.walk(path):
                dirs[:] = [d for d in dirs if d not in SKIP_DIRS]
                for fname in sorted(filenames):
                    if fname.endswith(".md"):
                        files.append(os.path.join(root, fname))
        else:
            continue
        
        for filepath in files:
            total += 1
            errors = validate_file(filepath)
            if errors:
                failed += 1
                for e in errors:
                    all_errors.append(f"{filepath}: {e}")
            else:
                passed += 1
    
    print(f"\n{'='*60}")
    print(f"Frontmatter lint: {passed}/{total} passed, {failed} failed")
    print(f"{'='*60}\n")
    
    if all_errors:
        for e in all_errors:
            print(f"  FAIL: {e}")
        print()
        return 1
    
    return 0

if __name__ == "__main__":
    sys.exit(main())
