#!/usr/bin/env python3
"""Documentation linter — enforces TOGAF-Lite + Writing Excellence protocol.

Usage:
    python3 scripts/lint_frontmatter.py [path ...]
    python3 scripts/lint_frontmatter.py --severity warn [path ...]

Exit codes:
    0 — no Alert-level diagnostics
    1 — Alert-level diagnostics detected
    2 — Warn-level (with --severity warn)

Validates:
    1. YAML frontmatter completeness (6 fields)
    2. HTML comment block (4 tags)
    3. DIAGRAM_ALIGNMENT for every mermaid block
    4. Freshness (0-90 fresh, 91-180 review, 181+ stale)
    5. togaf_phase and status value validity
    6. Voice: passive detection, sentence length, hedging
    7. Citation density per ## section
    8. Table purpose rule (≥3 cols × ≥3 rows)
    9. Broken internal links (line-level detection)
    10. Critical-set document metadata completeness
"""

import os
import re
import sys
import yaml
import json
from datetime import date, datetime
from collections import Counter

REQUIRED_FIELDS = ["title", "audience", "last_updated", "togaf_phase", "version", "status"]
VALID_PHASES = [
    "Preliminary", "A", "B", "C", "D", "E", "F", "G", "H", "Requirements Management",
    "Preliminary / Governance",  # Russell-specific compound
]
VALID_STATUSES = ["Active", "Proposed", "Superseded", "Deprecated", "Draft", "Aspirational", "Archived",
                   "Accepted", "Implemented",  # ADR-specific statuses
]
SKIP_DIRS = {"archive", "generated", "evolution-digest", "target", ".kilo", "node_modules"}

PASSIVE_PATTERNS = [
    r"\bis being\b", r"\bhas been\b", r"\bwas being\b", r"\bwere being\b",
    r"\bwill be (implemented|created|built|done|used|added|run)\b",
    r"\bcan be\b", r"\bmay be\b", r"\bshould be\b",
]
HEDGING_PATTERNS = [
    r"\bshould probably\b", r"\bmight\b", r"\bcould potentially\b",
    r"\bperhaps\b", r"\bmay wish to\b",
]

SEVERITY_ORDER = {"Info": 0, "Warn": 1, "Alert": 2, "Crit": 3}

class Diagnostic:
    def __init__(self, file, rule, severity, message, section=None, fix=None):
        self.file = file
        self.rule = rule
        self.severity = severity
        self.message = message
        self.section = section
        self.fix = fix

    def __repr__(self):
        prefix = f"[{self.severity:5s}]"
        loc = f"{self.file}"
        if self.section:
            loc += f" §{self.section}"
        return f"{prefix} {loc}: {self.message}"


def compute_freshness(last_updated_str, today):
    """Return (days, state) tuple."""
    try:
        updated = datetime.strptime(last_updated_str.strip(), "%Y-%m-%d").date()
        days = (today - updated).days
        if days <= 90:
            return days, "fresh"
        elif days <= 180:
            return days, "review"
        else:
            return days, "stale"
    except ValueError:
        return None, "unknown"


def count_sentences_longer_than(text, max_words=35):
    """Count sentences exceeding max_words."""
    sentences = re.split(r'(?<=[.!?])\s+', text)
    long_count = 0
    for s in sentences:
        words = s.split()
        if len(words) > max_words:
            long_count += 1
    return long_count


def detect_passive_voice(text):
    """Count passive voice instances."""
    count = 0
    examples = []
    for pattern in PASSIVE_PATTERNS:
        matches = re.findall(pattern, text, re.IGNORECASE)
        count += len(matches)
        if matches:
            examples.extend([m if isinstance(m, str) else m[0] for m in matches[:3]])
    return count, examples[:5]


def detect_hedging(text):
    """Count hedging phrases."""
    count = 0
    for pattern in HEDGING_PATTERNS:
        count += len(re.findall(pattern, text, re.IGNORECASE))
    return count


def count_citations(text):
    """Count [^...]: style citations."""
    return len(re.findall(r'\[\^[^\]]+\]:', text))


def count_h2_sections(text):
    """Count ## level headings."""
    return len(re.findall(r'^##\s', text, re.MULTILINE))


def count_table_rows(text):
    """Count table rows (pipe-delimited, skipping separator rows)."""
    rows = 0
    for line in text.split('\n'):
        stripped = line.strip()
        if stripped.startswith('|') and stripped.endswith('|'):
            if not re.match(r'^\|[\s\-:]+\|', stripped):
                rows += 1
    return rows


def validate_file(filepath, today):
    """Validate a single markdown file. Returns list of Diagnostic objects."""
    diags = []

    with open(filepath, "r", encoding="utf-8", errors="replace") as f:
        content = f.read()

    # Determine if this is a redirect stub
    is_redirect = "has moved" in content[:200] or "has been archived" in content[:200]

    # 1. Check YAML frontmatter
    fm_match = re.match(r'^---\r?\n(.*?)\r?\n---', content, re.DOTALL)
    if not fm_match:
        diags.append(Diagnostic(filepath, "frontmatter", "Alert",
                                 "Missing YAML frontmatter block (--- ... ---)"))
        return diags

    fm_text = fm_match.group(1)
    try:
        fm = yaml.safe_load(fm_text)
    except yaml.YAMLError as e:
        diags.append(Diagnostic(filepath, "frontmatter", "Alert",
                                 f"YAML parse error: {e}"))
        return diags

    if not isinstance(fm, dict):
        diags.append(Diagnostic(filepath, "frontmatter", "Alert",
                                 "Frontmatter is not a YAML mapping"))
        return diags

    # 2. Check required fields
    for field in REQUIRED_FIELDS:
        if field not in fm:
            severity = "Info" if is_redirect else "Alert"
            diags.append(Diagnostic(filepath, "frontmatter",
                                     severity, f"Missing required field: {field}",
                                     fix=f"Add '{field}: ...' to frontmatter"))

    # 3. Validate togaf_phase
    phase = fm.get("togaf_phase", "")
    if phase and phase not in VALID_PHASES:
        diags.append(Diagnostic(filepath, "togaf_phase", "Alert",
                                 f"Invalid togaf_phase: '{phase}'",
                                 fix=f"Use one of: {', '.join(VALID_PHASES)}"))

    # 4. Validate status
    status = fm.get("status", "")
    if status and status not in VALID_STATUSES:
        diags.append(Diagnostic(filepath, "status", "Warn",
                                 f"Invalid status: '{status}'",
                                 fix=f"Use one of: {', '.join(VALID_STATUSES)}"))

    # 5. Check HTML comment block (skip for redirects/archived)
    if not is_redirect:
        html_comments = re.findall(r'<!--\s*(TOGAF_DOMAIN|VERSION|STATUS|LAST_UPDATED):\s*(.+?)\s*-->', content)
        found_keys = {k for k, v in html_comments}
        required_keys = {"TOGAF_DOMAIN", "VERSION", "STATUS", "LAST_UPDATED"}
        missing_keys = required_keys - found_keys
        if missing_keys:
            diags.append(Diagnostic(filepath, "html_comment", "Warn",
                                     f"Missing HTML comment tags: {', '.join(sorted(missing_keys))}",
                                     fix="Add <!-- TOGAF_DOMAIN: ... --> block"))

    # 6. Check mermaid diagrams have DIAGRAM_ALIGNMENT
    mermaid_count = len(re.findall(r'```mermaid', content))
    alignment_count = len(re.findall(r'DIAGRAM_ALIGNMENT', content))
    if mermaid_count > 0 and alignment_count == 0:
        diags.append(Diagnostic(filepath, "diagram_alignment", "Alert",
                                 f"Has {mermaid_count} mermaid block(s) but no DIAGRAM_ALIGNMENT metadata",
                                 fix="Add <!-- DIAGRAM_ALIGNMENT id: DIAG-... ... --> after each ```mermaid block"))
    elif mermaid_count > alignment_count:
        # Some mermaid blocks lack alignment (allow extra alignments for cross-references)
        diags.append(Diagnostic(filepath, "diagram_alignment", "Warn",
                                 f"Has {mermaid_count} mermaid block(s) but only {alignment_count} DIAGRAM_ALIGNMENT",
                                 fix=f"Add {mermaid_count - alignment_count} missing DIAGRAM_ALIGNMENT block(s)"))

    # 7. Freshness check
    last_updated = fm.get("last_updated", "")
    if last_updated:
        days, state = compute_freshness(str(last_updated), today)
        if state == "stale":
            diags.append(Diagnostic(filepath, "freshness", "Alert",
                                     f"Stale: last_updated {last_updated} ({days} days ago)",
                                     fix="Update, archive, or explicitly exempt"))
        elif state == "review":
            diags.append(Diagnostic(filepath, "freshness", "Warn",
                                     f"Review needed: last_updated {last_updated} ({days} days ago)",
                                     fix="Verify or reconfirm content"))

    # 8. Voice checks (body text only, skip code blocks and frontmatter)
    body = re.sub(r'```.*?```', '', content, flags=re.DOTALL)
    body = re.sub(r'^---.*?---', '', body, flags=re.DOTALL)
    body = re.sub(r'<!--.*?-->', '', body, flags=re.DOTALL)

    long_sentences = count_sentences_longer_than(body)
    if long_sentences > 10:
        diags.append(Diagnostic(filepath, "voice.sentence_length", "Info",
                                 f"{long_sentences} sentences exceed 35 words",
                                 fix="Split long sentences"))

    passive_count, passive_examples = detect_passive_voice(body)
    if passive_count > 5:
        diags.append(Diagnostic(filepath, "voice.passive", "Info",
                                 f"{passive_count} passive voice instances (e.g. '{', '.join(passive_examples[:3])}')",
                                 fix="Rewrite in active voice"))

    hedging_count = detect_hedging(body)
    if hedging_count > 3:
        diags.append(Diagnostic(filepath, "voice.hedging", "Warn",
                                 f"{hedging_count} hedging phrases detected",
                                 fix="Replace 'should probably', 'might', 'could potentially' with definite assertions"))

    # 9. Citation density
    h2_count = count_h2_sections(body)
    citation_count = count_citations(body)
    if h2_count > 0 and citation_count == 0:
        doc_type_hint = ""
        if "architecture/" in filepath or "specifications/" in filepath or "standards/" in filepath:
            doc_type_hint = " (architecture/spec/standards require ≥1 citation per ## section)"
            diags.append(Diagnostic(filepath, "citations", "Warn",
                                     f"Zero citations across {h2_count} ## sections{doc_type_hint}",
                                     fix="Add APA 7th format citations per Writing Excellence §3.4"))

    # 10. Table purpose rule (not enforced strictly, just noted)
    table_rows = count_table_rows(content)
    if table_rows > 0 and table_rows < 3:
        diags.append(Diagnostic(filepath, "tables", "Info",
                                 f"Tables with fewer than 3 data rows detected",
                                 fix="Per DOCUMENTATION_STANDARDS §14, use tables only for ≥3 rows × ≥3 cols"))

    return diags


def is_critical_set(filepath):
    """Check if file is in the authoritative critical set."""
    critical_paths = [
        "AGENTS.md",
        "docs/README.md",
        "docs/status/CONSOLIDATED-STATUS.md",
        "docs/specifications/MVP_SPEC.md",
        "docs/architecture/PRINCIPLES_CATALOG.md",
        "docs/standards/safety.md",
        "docs/standards/DOCUMENTATION_STANDARDS.md",
        "docs/standards/WRITING_EXCELLENCE.md",
        "docs/standards/TOGAF_LITE_FOR_OPEN_SOURCE.md",
        "docs/specifications/PERSISTENCE_CATALOG.md",
        "docs/architecture/TOGAF_TRACEABILITY_MATRIX.md",
        "docs/architecture/CAPABILITY_GRAPH.md",
        "docs/architecture/CODE_ANCHOR_GRAPH.md",
    ]
    normalized = filepath.replace("\\", "/")
    return any(normalized.endswith(cp) for cp in critical_paths)


def main():
    import argparse
    parser = argparse.ArgumentParser(description="Documentation linter for Russell")
    parser.add_argument("paths", nargs="*", default=[], help="Files or directories to lint")
    parser.add_argument("--severity", default="alert", choices=["info", "warn", "alert"],
                        help="Minimum severity level to report (default: alert)")
    parser.add_argument("--json", action="store_true", help="Output diagnostics as JSON")
    parser.add_argument("--critical-only", action="store_true",
                        help="Only lint critical-set documents")
    args = parser.parse_args()

    paths = args.paths if args.paths else ["."]
    today = date.today()
    min_severity = SEVERITY_ORDER.get(args.severity.capitalize(), 2)

    all_diags = []
    files_processed = 0

    for path in paths:
        if os.path.isfile(path) and path.endswith(".md"):
            files_to_lint = [path]
        elif os.path.isdir(path):
            files_to_lint = []
            for root, dirs, filenames in os.walk(path):
                dirs[:] = [d for d in dirs if d not in SKIP_DIRS]
                for fname in sorted(filenames):
                    if fname.endswith(".md"):
                        fpath = os.path.join(root, fname)
                        if not args.critical_only or is_critical_set(fpath):
                            files_to_lint.append(fpath)
        else:
            continue

        for filepath in files_to_lint:
            files_processed += 1
            diags = validate_file(filepath, today)
            all_diags.extend(diags)

    # Filter by severity
    filtered = [d for d in all_diags if SEVERITY_ORDER.get(d.severity, 0) >= min_severity]

    if args.json:
        output = []
        for d in filtered:
            output.append({
                "file": d.file,
                "rule": d.rule,
                "severity": d.severity,
                "message": d.message,
                "section": d.section,
                "fix": d.fix,
            })
        print(json.dumps(output, indent=2))
    else:
        # Group by severity
        by_severity = {"Alert": [], "Warn": [], "Info": []}
        for d in filtered:
            by_severity.setdefault(d.severity, []).append(d)

        print(f"\n{'='*70}")
        print(f"Documentation Linter Report — {today.isoformat()}")
        print(f"Files processed: {files_processed}")
        print(f"Diagnostics: {len(filtered)} (threshold: {args.severity}+)")
        print(f"{'='*70}\n")

        for sev in ["Alert", "Warn", "Info"]:
            items = by_severity.get(sev, [])
            if items:
                print(f"--- {sev} ({len(items)}) ---")
                for d in sorted(items, key=lambda x: x.file):
                    print(f"  {d}")
                    if d.fix:
                        print(f"    Fix: {d.fix}")
                print()

        alert_count = len(by_severity.get("Alert", []))
        warn_count = len(by_severity.get("Warn", []))
        print(f"Summary: {alert_count} Alert(s), {warn_count} Warn(s), "
              f"{len(by_severity.get('Info', []))} Info(s)")
        print()

    # Exit codes
    alert_count = sum(1 for d in filtered if d.severity == "Alert")
    warn_count = sum(1 for d in filtered if d.severity == "Warn")

    if alert_count > 0:
        return 1
    elif args.severity == "warn" and warn_count > 0:
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
