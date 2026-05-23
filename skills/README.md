# Russell Skills — hKask Universal Agentic Registry

**Replicant ID**: `russell`  
**Visibility**: public-except-episodic  
**Total Skills**: 13  

## hKask Artifact Structure

Each Russell skill includes three artifact types for hKask registry compatibility:

| File | Purpose | Format |
|---|---|---|
| `manifest.yaml` | Russell native | YAML |
| `SKILL.md` | hKask universal skill | Markdown (OpenClaw-compatible) |
| `skill.json` | hKask descriptor | JSON (SchemaStore-registered) |

## Skills Catalog

| Skill | Status | Symptoms | Probes | Interventions |
|---|---|---|---|---|
| `okapi-watcher` | ✅ Complete | 3 | 3 | 1 |
| `skill-manager` | ✅ Complete | 6 | 3 | 6 |
| `skill-workshop` | ✅ Placeholder | 2 | 2 | 4 |
| `skill-maintenance` | ✅ Placeholder | 2 | 2 | 2 |
| `skill-discovery` | ✅ Placeholder | 2 | 2 | 3 |
| `sysadmin` | ✅ Placeholder | 3 | 2 | 3 |
| `scenario-tester` | ✅ Placeholder | 1 | 7 | 0 |
| `journal-compactor` | ✅ Placeholder | 1 | 1 | 1 |
| `pragmatic-cybernetics` | ✅ Placeholder | 1 | 0 | 0 |
| `pragmatic-semantics` | ✅ Placeholder | 1 | 0 | 0 |
| `ubuntu-jack` | ✅ Placeholder | 1 | 2 | 1 |
| `web-search` | ✅ Placeholder | 1 | 1 | 0 |
| `package-checker` | ✅ Placeholder | 1 | 1 | 0 |

## Templates

Skill templates in `templates/` directory:

- `russell-skill.yaml.j2` — Russell native manifest
- `SKILL.md.j2` — hKask universal skill
- `skill.json.j2` — hKask descriptor

## Publishing to hKask

```bash
# Lint skill package
npx @hol-org/registry skills lint --dir ./skills/okapi-watcher

# Verify against schema
npx @hol-org/registry skills verify --name "okapi-watcher" --tier basic

# Publish to registry
npx @hol-org/registry skills publish --dir ./skills/okapi-watcher --account-id 0.0.1234
```

## Replicant Metadata

All Russell skills include:

```json
{
  "replicant": {
    "id": "russell",
    "visibility": "public",
    "artifact_type": "semantic_memory",
    "episodic_memory": false
  }
}
```

## References

- [hKask Integration Guide](../../docs/standards/hkask-integration.md)
- [REPLICANT.md](../../REPLICANT.md)
- [HCS-26 Standard](https://github.com/hiero-ledger/hiero-consensus-specifications/blob/main/docs/standards/hcs-26.md)
