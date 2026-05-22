# Russell ACP Integration — Ready for Deployment

**Date:** 2026-05-22  
**Status:** ✅ Phase 2 Complete — Ready for hKask Integration

---

## What's Ready

### Binary
- **Location:** `target/release/russell-acp-server`
- **Size:** ~15 MB optimized
- **Tests:** 9/9 passing

### Deployment Scripts
- `docs/deployment/install.sh` — Automated installation
- `docs/deployment/macaroon-setup.sh` — Macaroon configuration
- `docs/deployment/QUICKSTART.md` — 5-minute setup guide

### Systemd Units
- `russell-acp-server.service` — ACP server (long-running)
- `russell-sentinel.timer` — Host probes (5-min cadence)
- `russell-sentinel.service` — Sentinel oneshot

### Documentation
- `docs/deployment/acp-integration.md` — Full integration guide
- `docs/deployment/INSTALL.md` — Installation reference
- `docs/plans/ACP-INTEGRATION-SUMMARY.md` — Status tracking
- `docs/adr/0026-acp-integration.md` — Architecture decisions

---

## Quick Deploy

```bash
# 1. Install
./docs/deployment/install.sh

# 2. Configure macaroon
./docs/deployment/macaroon-setup.sh
source ~/.bashrc

# 3. Enable services
systemctl --user enable --now russell-sentinel.timer
systemctl --user enable --now russell-acp-server.service

# 4. Verify
systemctl --user status russell-acp-server.service
```

---

## Security Boundary Summary

| Exposed | Protected |
|---------|-----------|
| 8 public skills | 6 private skills |
| Read-only probes | Sudo interventions |
| Journal viewer | Proprioception vitals |
| Web search | Okapi management |
| Scenario tests | Skill registry mutations |

---

## Next: hKask Integration

1. **Register Russell in hKask config** — Add ACP agent entry
2. **Test bidirectional communication** — hKask → Russell probes
3. **Verify graceful degradation** — Russell operates during hKask outage
4. **Security audit** — Penetration test ACP surface

---

## Compliance Checklist

- [x] ADR-0026 implemented
- [x] JR-1 through JR-7 compliant
- [x] ADR-0025 (MCP client) constraints honored
- [x] 8 public skills with hLexicon metadata
- [x] 6 private skills protected
- [x] Macaroon OCAP authentication
- [x] Rate limiting (100/min)
- [x] Evidence logging to journal
- [x] Systemd deployment units
- [x] Installation scripts
- [x] Documentation complete

---

**Contact:** Russell Team  
**Review Date:** After hKask integration testing
