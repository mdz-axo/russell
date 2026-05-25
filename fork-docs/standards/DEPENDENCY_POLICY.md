---
title: "Russell Dependency Policy"
audience: [contributors, maintainers]
last_updated: 2026-05-25
version: "1.0.0"
status: "Active"
domain: "Cross-cutting"
ddmvss_categories: [trust, lifecycle]
---

# Russell Dependency Policy

**Purpose:** Define how Russell manages external dependencies.

**Axiom:** *Reuse, don't depend.* (JR-6)

---

## 1. Dependency Philosophy

### 1.1 Copy Over Depend

Russell prefers to **copy** code from upstream workspaces rather than **depend** on them via Cargo.

**Rationale:**
- Eliminates version conflicts
- Reduces build complexity
- Improves auditability
- Enables customization

**Implementation:**
- Copy source files into `crates/<crate-name>/vendor/`
- Document source in `REUSE_MANIFEST.md`
- Sync manually when upstream updates

### 1.2 Minimal Dependencies

When dependencies are necessary, prefer:
- **Standard library** over external crates
- **Mature crates** (>1 year, >1000 downloads) over new ones
- **Single-purpose crates** over frameworks
- **Local crates** over external crates

---

## 2. Allowed Dependencies

### 2.1 Core Dependencies

These dependencies are allowed without ADR:

| Crate | Purpose | Justification |
|-------|---------|---------------|
| `serde` | Serialization | Industry standard |
| `tokio` | Async runtime | Industry standard |
| `tracing` | Logging | Industry standard |
| `clap` | CLI parsing | Industry standard |
| `rusqlite` | SQLite | Industry standard |
| `ulid` | IDs | Industry standard |
| `chrono` | Timestamps | Industry standard |
| `anyhow` | Error handling | Industry standard |
| `thiserror` | Error types | Industry standard |

### 2.2 Optional Dependencies

These dependencies require ADR:

| Crate | Purpose | ADR |
|-------|---------|-----|
| `reqwest` | HTTP client | ADR-0008 |
| `macaroon` | OCAP tokens | ADR-0027 |
| `jsonrpc-core` | JSON-RPC | ADR-0027 |
| `landlock` | Sandboxing | ADR-0024 |

### 2.3 Forbidden Dependencies

These dependencies are forbidden:

| Crate | Reason |
|-------|--------|
| `openssl` | Use `rustls` instead |
| `curl` | Use `reqwest` instead |
| `gtk` | No GUI |
| `winit` | No GUI |
| `bevy` | No game engine |

---

## 3. Dependency Management

### 3.1 Adding a Dependency

1. Check if standard library provides equivalent
2. Check if existing dependency provides equivalent
3. Check if copying is feasible (JR-6)
4. If dependency required, file ADR with:
   - Purpose
   - Alternatives considered
   - Justification
5. Add to `Cargo.toml` with version constraint
6. Update this document if adding to allowed list

### 3.2 Updating a Dependency

1. Check changelog for breaking changes
2. Run `cargo update -p <crate>`
3. Run `cargo test --workspace`
4. Run `cargo clippy --workspace`
5. Commit with message: `deps: update <crate> to <version>`

### 3.3 Removing a Dependency

1. Remove from `Cargo.toml`
2. Replace with standard library or copy
3. Run `cargo test --workspace`
4. Commit with message: `deps: remove <crate>`

---

## 4. Security Policy

### 4.1 Vulnerability Response

When a vulnerability is disclosed:

1. Check if Russell uses affected version
2. If yes, update to patched version
3. If no patch available, evaluate risk:
   - **Critical:** Remove dependency immediately
   - **High:** Remove dependency within 7 days
   - **Medium:** Remove dependency within 30 days
   - **Low:** Document risk, plan removal
4. File ADR documenting decision

### 4.2 Audit Process

Quarterly dependency audit:

```bash
cargo audit
cargo deny check
```

**Action items:**
- Fix all critical vulnerabilities immediately
- Fix all high vulnerabilities within 7 days
- Document medium/low vulnerabilities in ADR

### 4.3 Supply Chain Security

- Pin all dependencies to specific versions
- Use `Cargo.lock` in version control
- Verify checksums with `cargo verify-project`
- Review new dependencies before adding

---

## 5. License Compliance

### 5.1 Allowed Licenses

These licenses are allowed without ADR:

- MIT
- Apache-2.0
- BSD-2-Clause
- BSD-3-Clause
- ISC
- Zlib
- Unlicense

### 5.2 Restricted Licenses

These licenses require ADR:

- MPL-2.0 (file-level copyleft)
- LGPL (weak copyleft)
- GPL (strong copyleft)

### 5.3 Forbidden Licenses

These licenses are forbidden:

- AGPL (network copyleft)
- SSPL (service copyleft)
- Proprietary licenses

### 5.4 License Check

```bash
cargo deny check licenses
```

**Action items:**
- Remove dependencies with forbidden licenses
- File ADR for dependencies with restricted licenses
- Document all license decisions

---

## 6. Reuse Manifest

### 6.1 Structure

`REUSE_MANIFEST.md` documents all copied code:

```markdown
## <crate-name>

**Source:** <upstream-repo-url>  
**Commit:** <commit-sha>  
**License:** <license>  
**Files:**
- `vendor/<file1>.rs`
- `vendor/<file2>.rs`

**Modifications:**
- <modification-1>
- <modification-2>

**Sync policy:** Manual sync when upstream fixes critical bug
```

### 6.2 Sync Process

1. Check upstream for updates
2. Review changelog
3. Copy updated files
4. Reapply modifications
5. Run `cargo test --workspace`
6. Update `REUSE_MANIFEST.md` with new commit SHA
7. Commit with message: `vendor: sync <crate-name> to <commit-sha>`

---

## 7. Dependency Metrics

### 7.1 Track These Metrics

- **Total dependencies:** Count of direct + transitive dependencies
- **Dependency depth:** Maximum depth of dependency tree
- **License distribution:** Count by license type
- **Vulnerability count:** Count by severity
- **Stale dependencies:** Dependencies not updated in 180+ days

### 7.2 Targets

- **Total dependencies:** ≤ 50 direct, ≤ 200 transitive
- **Dependency depth:** ≤ 5
- **Vulnerabilities:** 0 critical, 0 high
- **Stale dependencies:** 0

### 7.3 Reporting

Quarterly dependency report:

```bash
cargo tree --depth 1 | wc -l  # Direct dependencies
cargo tree | wc -l  # Total dependencies
cargo audit  # Vulnerabilities
cargo deny check licenses  # License compliance
```

---

## 8. Tooling

### 8.1 Dependency Analysis

```bash
# List all dependencies
cargo tree

# List dependencies by license
cargo deny check licenses

# Check for vulnerabilities
cargo audit

# Check for outdated dependencies
cargo outdated

# Check for unused dependencies
cargo machete
```

### 8.2 Dependency Updates

```bash
# Update all dependencies
cargo update

# Update specific dependency
cargo update -p <crate>

# Check for breaking changes
cargo semver-checks
```

---

## 9. References

- JR-6 (Reuse, don't depend)
- ADR-0013 (Rust workspace layout)
- ADR-0017 (Reuse over dependency)
- hKask Dependency Policy: `~/Clones/hKask/docs/standards/DEPENDENCY_POLICY.md`
