---
title: "Container Runtime — Architecture & Configuration"
audience: [operators, developers]
last_updated: 2026-05-07
togaf_phase: "D"
version: "1.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Technology -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-07 -->


# Container Runtime — Architecture & Configuration

> Last updated: 2026-05-07

## Summary

This workstation uses **Podman 5.4.x rootless** as the container
engine. There is no Docker daemon. The Docker CLI (`docker-ce-cli`)
is retained for compatibility and routes to Podman via its user
socket.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│  User Space (rootless, no sudo needed)                  │
│                                                         │
│  docker CLI ──→ podman.socket ──→ Podman engine         │
│  podman CLI ──────────────────→ Podman engine           │
│                                                         │
│  Containers managed as systemd user services (Quadlet)  │
│  Survive logout via loginctl linger                     │
│  Auto-start on boot via [Install] WantedBy=default      │
└─────────────────────────────────────────────────────────┘
```

## Installed Packages

| Package | Role |
|---------|------|
| `podman` 5.4.2 | Container engine (rootless) |
| `crun` 1.21 | OCI runtime (faster than runc for rootless) |
| `conmon` 2.1.12 | Container monitor |
| `netavark` 1.14.0 | Container networking |
| `docker-ce-cli` 29.4.3 | Docker CLI (client only, no daemon) |
| `docker-buildx-plugin` 0.33.0 | BuildKit builder |
| `docker-compose-v2` 2.40.3 | Compose (works with Podman socket) |
| `podman-compose` 1.3.0 | Native Podman compose alternative |

**Not installed (intentionally):**
- `docker-ce` — no Docker daemon needed
- `containerd` — Podman uses crun directly
- `podman-docker` — using Docker context instead

## Running Containers

### kask-qdrant

| Field | Value |
|-------|-------|
| Image | `docker.io/qdrant/qdrant:latest` |
| Purpose | Vector database for the hKask knowledge system |
| Ports | 6333 (HTTP API), 6334 (gRPC) |
| Volume | `~/Clones/kask/.data/qdrant` → `/qdrant/storage` |
| Env | `RUN_MODE=production` |
| Management | Quadlet systemd user service |
| Auto-update | Yes (registry pull via `podman-auto-update.timer`) |

Quadlet file: `~/.config/containers/systemd/kask-qdrant.container`

## Key Configuration

| Setting | Value | Why |
|---------|-------|-----|
| Linger | enabled | User services survive logout |
| podman.socket | enabled (user) | Docker CLI compatibility |
| Docker context | `podman` → `/run/user/1000/podman/podman.sock` | Routes `docker` commands to Podman |
| Auto-update timer | enabled | Keeps images fresh |

## Common Operations

```sh
# Status
systemctl --user status kask-qdrant
podman ps
docker ps                              # same thing

# Logs
podman logs kask-qdrant
podman logs -f kask-qdrant             # follow

# Restart
systemctl --user restart kask-qdrant

# Manual image update
podman auto-update

# Add a new container
# 1. Write a .container file in ~/.config/containers/systemd/
# 2. systemctl --user daemon-reload
# 3. systemctl --user enable --now <name>
```

## Why Not Docker CE?

1. **Rootless by default** — no privileged daemon, better security.
2. **Systemd-native** — Quadlet files are just systemd units.
3. **No daemon** — containers are direct child processes, no
   restart-the-world when updating the engine.
4. **Docker CLI still works** — via the socket, zero workflow change.
5. **Ubuntu 25.10 ships Podman** — it's the distro-supported path.

## Relationship to Russell

Russell's health check (`system-health-check.sh`) should be
updated to check Podman status rather than Docker daemon
connectivity. The relevant probe is:

- Is `podman.socket` active?
- Is `kask-qdrant.service` running?
- Can we reach Qdrant at `localhost:6333`?

This is tracked for the Sentinel's container probe (post-MVP).
