#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# Check for broken internal markdown links in docs/

set -e

DOCS_DIR="${1:-docs}"
BROKEN=0

echo "Checking internal links in $DOCS_DIR..."

# Find all markdown files (excluding archive)
for file in $(find "$DOCS_DIR" -name "*.md" -not -path "*/archive/*" -type f); do
    # Extract relative links (starting with ./ or ../ or just path without protocol)
    links=$(grep -oE '\]\([^)]+\)' "$file" | sed 's/]//;s/(//;s/)//' || true)
    
    for link in $links; do
        # Skip external URLs
        if [[ "$link" == http* ]]; then
            continue
        fi
        
        # Skip anchor-only links
        if [[ "$link" == \#* ]]; then
            continue
        fi
        
        # Skip parameter links (c=0.85 style)
        if [[ "$link" == c=* ]]; then
            continue
        fi
        
        # Resolve relative to current file's directory
        dir=$(dirname "$file")
        if [[ "$link" != /* ]]; then
            target="$dir/$link"
        else
            target="$link"
        fi
        
        # Remove anchor suffix if present
        target="${target%%#*}"
        
        # Check if target is a directory (link like `adr/`)
        if [[ -d "$target" ]]; then
            # Check if directory has a README.md
            if [[ -f "$target/README.md" ]]; then
                continue
            else
                echo "MISSING README: $file -> $target/"
                BROKEN=$((BROKEN + 1))
                continue
            fi
        fi
        
        # Check if file exists
        if [[ ! -f "$target" ]]; then
            echo "BROKEN: $file -> $target"
            BROKEN=$((BROKEN + 1))
        fi
    done
done

if [[ $BROKEN -gt 0 ]]; then
    echo ""
    echo "Found $BROKEN broken link(s)"
    exit 1
else
    echo "All internal links valid"
    exit 0
fi
