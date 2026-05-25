---
title: "Russell CI/CD Guide"
audience: [developers, maintainers]
last_updated: 2026-05-25
version: "1.0.0"
status: "Active"
domain: "Cross-cutting"
ddmvss_categories: [lifecycle]
---

# Russell CI/CD Guide

**Purpose:** Define Russell's continuous integration and continuous deployment pipeline.

---

## 1. CI Pipeline

### 1.1 Triggers

- **Push to main:** Run full pipeline
- **Pull request:** Run full pipeline
- **Tag (v*):** Run full pipeline + release

### 1.2 Stages

#### Stage 1: Build

```bash
cargo check --workspace
cargo build --workspace --release
```

**Success criteria:** No compilation errors.

#### Stage 2: Test

```bash
cargo test --workspace
```

**Success criteria:** All tests pass.

#### Stage 3: Lint

```bash
cargo clippy --workspace -- -D warnings
cargo fmt --check
```

**Success criteria:** No warnings, no formatting diffs.

#### Stage 4: Security

```bash
cargo audit
cargo deny check
```

**Success criteria:** No critical/high vulnerabilities, no forbidden licenses.

#### Stage 5: Documentation

```bash
# Check internal links
find docs -name '*.md' -exec grep -l '\[.*\](.*)' {} \; | xargs -I {} sh -c 'echo "Checking {}"; grep -oP '\[.*?\]\(\K[^)]+' {} | while read link; do if [[ ! "$link" =~ ^http ]]; then target=$(dirname {})/$link; if [ ! -f "$target" ]; then echo "BROKEN: $link in {}"; exit 1; fi; fi; done'

# Validate frontmatter
for f in $(find docs -name '*.md'); do
  head -20 "$f" | grep -q 'ddmvss_categories:' || (echo "MISSING ddmvss_categories: $f" && exit 1)
done

# Validate diagrams
for f in $(find docs -name '*.md'); do
  if grep -q '```mermaid' "$f"; then
    grep -q 'DIAGRAM_ALIGNMENT' "$f" || (echo "MISSING DIAGRAM_ALIGNMENT: $f" && exit 1)
  fi
done
```

**Success criteria:** No broken links, all frontmatter valid, all diagrams have alignment metadata.

### 1.3 Caching

- **Cargo registry:** Cache `~/.cargo/registry`
- **Cargo git:** Cache `~/.cargo/git`
- **Target directory:** Cache `target/` (except `target/debug/deps`)

### 1.4 Parallelism

- Build and test run in parallel
- Lint and security run in parallel
- Documentation runs after build (needs compiled binaries for some checks)

---

## 2. CD Pipeline

### 2.1 Triggers

- **Tag (v*):** Deploy release

### 2.2 Stages

#### Stage 1: Build Release

```bash
cargo build --release --target x86_64-unknown-linux-gnu
```

**Artifacts:**
- `target/release/russell` (binary)
- `target/release/russell-acp-server` (binary)

#### Stage 2: Package

```bash
# Create tarball
tar -czf russell-${VERSION}-x86_64-linux.tar.gz \
  -C target/release \
  russell \
  russell-acp-server

# Create checksum
sha256sum russell-${VERSION}-x86_64-linux.tar.gz > russell-${VERSION}-x86_64-linux.tar.gz.sha256
```

**Artifacts:**
- `russell-${VERSION}-x86_64-linux.tar.gz`
- `russell-${VERSION}-x86_64-linux.tar.gz.sha256`

#### Stage 3: Publish

```bash
# Create GitHub release
gh release create ${VERSION} \
  --title "Russell ${VERSION}" \
  --notes "Release notes here" \
  russell-${VERSION}-x86_64-linux.tar.gz \
  russell-${VERSION}-x86_64-linux.tar.gz.sha256
```

**Success criteria:** Release created on GitHub.

---

## 3. Local Development

### 3.1 Pre-Commit Checks

```bash
# Run before committing
cargo fmt
cargo clippy --workspace -- -D warnings
cargo test --workspace
cargo audit
```

### 3.2 Pre-Push Checks

```bash
# Run before pushing
cargo build --workspace --release
```

### 3.3 Git Hooks

Install pre-commit hook:

```bash
cat > .git/hooks/pre-commit << 'EOF'
#!/bin/bash
set -e
cargo fmt --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
EOF
chmod +x .git/hooks/pre-commit
```

---

## 4. Monitoring

### 4.1 CI Metrics

Track these metrics:

- **Build time:** Time to compile
- **Test time:** Time to run tests
- **Test coverage:** % of code covered by tests
- **Flaky tests:** Count of tests that fail intermittently
- **CI failures:** Count of CI failures per week

### 4.2 Alerts

Alert on:

- **CI failure:** Main branch CI fails
- **Security vulnerability:** Critical/high vulnerability detected
- **Broken links:** Documentation link check fails

---

## 5. Troubleshooting

### 5.1 Common Issues

**Issue:** `cargo build` fails with "cannot find -lsqlite3"  
**Solution:** Install `libsqlite3-dev` (Ubuntu) or `sqlite` (macOS)

**Issue:** `cargo test` fails with "permission denied"  
**Solution:** Check file permissions in `~/.local/state/harness/`

**Issue:** `cargo clippy` warns about "unused import"  
**Solution:** Remove unused import or add `#[allow(unused_imports)]` with justification

### 5.2 Debugging

Enable verbose output:

```bash
cargo build --verbose
cargo test --verbose
```

Enable backtraces:

```bash
RUST_BACKTRACE=1 cargo test
```

---

## 6. References

- GitHub Actions: https://docs.github.com/en/actions
- Cargo: https://doc.rust-lang.org/cargo/
- cargo-audit: https://github.com/rustsec/rustsec/tree/main/cargo-audit
- cargo-deny: https://github.com/EmbarkStudios/cargo-deny
