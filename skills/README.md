# Russell Skills — Agent Registry

**Replicant ID**: `russell`  
**Visibility**: public-except-episodic  
**Total Skills**: 13  

## Artifact Structure

Each Russell skill includes these artifact types:

| File | Purpose | Format |
|---|---|---|
| `manifest.yaml` | Russell native | YAML |
| `SKILL.md` | Skill documentation | Markdown |

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
- `SKILL.md.j2` — Skill documentation template



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

- [REPLICANT.md](../../REPLICANT.md)
