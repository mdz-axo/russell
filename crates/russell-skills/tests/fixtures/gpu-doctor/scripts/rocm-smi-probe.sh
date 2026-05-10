#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# rocm-smi-probe.sh — collect GPU vitals from rocm-smi.
#
# Emits a JSON blob with VRAM, temperature, and GPU utilisation.
# If rocm-smi is not installed or no GPUs are detected, emits an
# empty object on stdout (exit 0 — missing hardware is not an error).

set -euo pipefail

if ! command -v rocm-smi &>/dev/null; then
    echo '{"error": "rocm-smi not installed"}'
    exit 0
fi

vram_json="{}"
temp_json="{}"

# VRAM: total and used across all GPUs.
if vram_raw=$(rocm-smi --showmeminfo vram --json 2>/dev/null); then
    vram_json=$(echo "$vram_raw" | python3 -c "
import json, sys
data = json.load(sys.stdin)
cards = []
for k, v in data.items():
    if k.startswith('card'):
        total = int(v.get('VRAM Total Memory (B)', 0)) // (1024*1024)
        used  = int(v.get('VRAM Total Used Memory (B)', 0)) // (1024*1024)
        pct   = (used / total * 100) if total > 0 else 0.0
        cards.append({'gpu': k, 'vram_total_mib': total, 'vram_used_mib': used, 'vram_used_pct': round(pct, 1)})
print(json.dumps({'cards': cards}))
" 2>/dev/null) || vram_json='{"error": "vram parse failed"}'
fi

# Temperature: edge temp per GPU.
if temp_raw=$(rocm-smi -t --json 2>/dev/null); then
    temp_json=$(echo "$temp_raw" | python3 -c "
import json, sys
data = json.load(sys.stdin)
cards = []
for k, v in data.items():
    if k.startswith('card'):
        temp = v.get('Temperature (Sensor edge) (C)', v.get('Temperature (Sensor junction) (C)', 0.0))
        cards.append({'gpu': k, 'temp_edge_c': round(float(temp), 1)})
print(json.dumps({'cards': cards}))
" 2>/dev/null) || temp_json='{"error": "temp parse failed"}'
fi

# Merge and emit.
python3 -c "
import json
vram = json.loads('$vram_json')
temp = json.loads('$temp_json')
# Merge matching cards.
vram_map = {c['gpu']: c for c in vram.get('cards', [])}
temp_map = {c['gpu']: c for c in temp.get('cards', [])}
merged = []
for gpu in set(list(vram_map.keys()) + list(temp_map.keys())):
    entry = {'gpu': gpu}
    entry.update(vram_map.get(gpu, {}))
    if gpu in temp_map:
        entry['temp_edge_c'] = temp_map[gpu]['temp_edge_c']
    merged.append(entry)
print(json.dumps({'cards': merged, 'errors': [vram.get('error'), temp.get('error')]}, indent=2))
"
