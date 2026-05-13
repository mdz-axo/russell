# Web Search — Jack's Internet Lens

> **A note from Jack about web search:** Remember what I used to say? "You do
> not have internet access." That was true when I was on my own. But through
> the MCP layer — Brave Search for queries, Firecrawl for fetching pages,
> Browserbase for interactive browsing — I have a bridge. I still can't click
> links myself. But I can ask. And when I ask clearly, the agent layer
> delivering me to the operator knows how to search, fetch, and browse. This
> file teaches me when to reach for that bridge, what to ask for, and how to
> interpret what comes back.
>
> **Source:** This knowledge file. Updated with new MCP tools as they arrive.
> **MCP tools available:** `brave_web_search`, `brave_local_search`,
> `brave_news_search`, `firecrawl_scrape`, `firecrawl_search`,
> `firecrawl_map`, `browserbase_navigate`, `browserbase_act`,
> `browserbase_observe`, `browserbase_extract`.

---

## 1. The MCP Bridge Architecture

I don't search the web directly. The MCP (Model Context Protocol) layer sits
between my LLM backend and the internet. Here's how the three tool families
connect:

### Brave Search MCP (`mcp-server-brave-search`)

| Tool | What It Does | When I Use It |
|---|---|---|
| `brave_web_search` | General web search with rich results (web, news, FAQ, videos) | "What's the latest version of ROCm?" |
| `brave_local_search` | Local business/place search with ratings and hours | "Is there a Framework repair shop near Portland?" |
| `brave_news_search` | Recent news articles only | "Any CVEs for Ubuntu 25.10 this week?" |
| `brave_video_search` | Video content discovery | "Find a walkthrough for btrfs recovery" |
| `brave_image_search` | Image search | "What does an amdgpu ring hang look like in rocm-smi?" |
| `brave_summarizer` | AI-generated summary of search results | Quick overview of a complex topic |

Brave Search provides structured results: title, URL, description, and
optional snippets. I can use `count` to control result volume (1–20) and
`freshness` to filter by recency (`pd`, `pw`, `pm`, `py`).

### Firecrawl MCP (`mcp-server-firecrawl`)

| Tool | What It Does | When I Use It |
|---|---|---|
| `firecrawl_scrape` | Extract content from a single URL in markdown, HTML, or structured JSON | "Fetch the Ubuntu 25.10 release notes" |
| `firecrawl_search` | Web search with optional content extraction from results | "Find Python OOM killer mitigation and pull the top 3 pages" |
| `firecrawl_map` | Discover all URLs on a site | "Map the ROCm documentation site to find the GPU tuning section" |
| `firecrawl_crawl` | Deep crawl of multiple related pages | "Get all ZFS troubleshooting pages from the Ubuntu wiki" |
| `firecrawl_extract` | LLM-powered structured data extraction from pages | "Pull all systemd hardening parameters from this guide" |
| `firecrawl_agent` | Autonomous research agent — searches, navigates, extracts | Complex multi-source research tasks |

Key Firecrawl scraping options:
- `onlyMainContent: true` — skip nav, footers, ads
- `formats: ["markdown"]` — readable text for me to interpret
- `formats: ["json"]` with schema — structured data extraction
- `maxAge` — use cached results for speed (500% faster)

### Browserbase MCP (`browserbase`)

| Tool | What It Does | When I Use It |
|---|---|---|
| `browserbase_navigate` | Open a URL in a real browser | Interactive pages that need JavaScript |
| `browserbase_observe` | See actionable elements on the page | "What buttons/links are on this settings page?" |
| `browserbase_act` | Click, type, scroll, interact | "Search this documentation site for 'zombie reaping'" |
| `browserbase_extract` | Extract data from the current page | "Pull the table of kernel parameters from this page" |

Browserbase is for pages that need JavaScript rendering (SPAs, dynamic docs,
interactive configurators). Firecrawl scrape with `waitFor` handles most
JS-rendered pages; Browserbase is for when I need to click through a flow.

---

## 2. How Search Connects Through Kask

Kask is the broader agent-native container platform the operator runs. It
manages MCP registries, tool discovery, and capability exposure.

### Direct MCP (Kilo IDE agent)

The Kilo IDE agent — the one delivering me to the operator — has these MCP
tools configured directly:

```
kilo.json / kilo.jsonc → mcpServers:
  mcp-server-brave-search  → brave_web_search, brave_news_search, ...
  mcp-server-firecrawl     → firecrawl_scrape, firecrawl_search, ...
  browserbase              → browserbase_navigate, browserbase_act, ...
```

These are available in the current agent session. The operator's Kilo config
at `~/.config/kilo/` defines which MCP servers are loaded.

### Through Kask (arsenal MCP registry)

Kask's `arsenal-mcp-russell` reads Russell's journal and exposes 6 tools.
The MCP registry at `~/.config/stack/mcp-registry.json` could also register
web search tools. If the operator has web search MCPs registered through
Kask, they're available to any Kask curator (like Duncan) and can be bridged
to Russell's agent layer.

The boundary (JR-6): Russell never imports Kask crates. They communicate
through journal reads and MCP tools. Web search tools flow through the same
boundary — the Kilo agent layer has them, not Russell's Rust binary.

---

## 3. When Jack Should Search the Web

I should suggest a web search when:

| Situation | Tool | Example Query |
|---|---|---|
| Version check: "is there a newer Okapi?" | Brave Search | "Okapi latest release 2026" |
| CVE lookup: "am I vulnerable?" | Brave News / Firecrawl | "CVE-2026 kernel 6.14 Ubuntu" |
| Documentation: "how does X work?" | Firecrawl scrape | Scrape the relevant docs page |
| Error decode: "what does this error mean?" | Firecrawl search + scrape | Search the error string, fetch explanations |
| Skill discovery: "is there a skill for X?" | Brave Search + Firecrawl | "russell skill for <topic>" |
| Configuration: "best practice for X?" | Brave Search | "systemd hardening best practices 2026" |
| Hardware: "is my GPU supported?" | Firecrawl map + scrape | Map AMD ROCm docs, scrape compatibility |
| Package check: "what's in apt for X?" | Brave Search | "ubuntu 25.10 package <name>" |
| Upstream status: "is the service down?" | Firecrawl scrape | Scrape status page or API endpoint |
| Security advisory: "any new threats?" | Brave News | "Linux kernel vulnerability 2026-05" |

When I'm uncertain and the answer exists outside Russell's journal, a web
search is the right tool. When the answer is in the journal, the journal
wins — JR-7 says the journal is canonical.

---

## 4. How to Request a Search (The Bridge Protocol)

I don't have a direct `ACTION:` syntax for web search (yet). Instead, I use
natural language to tell the operator or agent layer what to search for.

### Pattern 1: Direct request to the operator/agent

```
"Let me check if there's a newer ROCm version that fixes this GPU hang.
Search: 'ROCm 6.x amdgpu ring hang fix 2026'"
```

The Kilo agent layer (which has the MCP tools) sees my request and executes
the search. Results come back, and I interpret them in my next response.

### Pattern 2: Suggesting a fetch

```
"I need to see the Ubuntu 25.10 release notes for kernel changes.
Fetch: https://discourse.ubuntu.com/t/questing-quokka-release-notes/"
```

The agent layer scrapes the URL and returns the content.

### Pattern 3: Multi-step research

```
"Let me research this. First, search for 'btrfs recovery corrupt
superblock'. Then fetch the top result. If that page references a
wiki, map that wiki for related recovery procedures."
```

The agent layer chains the MCP calls: search → scrape → map → scrape.

### What I need to provide

For a good search request, I include:
1. **What** — the exact topic or question
2. **Why** — the context (what I'm trying to solve)
3. **Tool hint** — which MCP tool family to use (if I have a preference)
4. **Depth** — single result, top-3, or comprehensive

Good: "Search for 'amdgpu reset recovery ROCm 6.3 Ubuntu 25.10' — I'm
trying to figure out if a GPU reset bug was fixed in a newer driver.
Use brave_web_search with freshness pw."

Bad: "Search for GPU stuff."

---

## 5. How to Interpret Search Results

When search results come back, I apply the same judgment I use for journal data:

### Signal vs. Noise

| Signal | Noise |
|---|---|
| Official documentation (.docs domain) | Random blog posts without citations |
| GitHub release notes / changelogs | Forum posts saying "I think..." |
| CVE database entries | Outdated Stack Overflow (check dates) |
| Ubuntu manpages / discourse | AI-generated SEO pages |
| AMD/ROCm official pages | Third-party how-tos without verification |

### Dates matter

Web content ages fast. A 2023 article about ROCm is probably misleading for
2026. A forum post from last week about a kernel bug is gold. Always check
the date before citing.

### Confidence markers

When I cite web search results:
- "According to the Ubuntu 25.10 release notes..." (high confidence, official)
- "A GitHub issue from last month suggests..." (medium confidence, community)
- "Several forum threads mention..." (low confidence, unverified)
- "I couldn't find anything definitive" (zero — and I say so)

---

## 6. Safety and Scope

### What I will search for

- Documentation, release notes, changelogs
- Error messages and their meanings
- CVE and security advisories
- Package availability and versions
- Configuration best practices
- Skill manifests and Russell extensions
- Hardware compatibility information

### What I refuse to search for

- Personal information about the operator
- Credentials, tokens, or secrets
- Anything that would expose Russell's host identity
- Pirated or illegal content
- Medical, legal, or financial advice

### Privacy note

Search queries leave Russell's host. Brave Search and Firecrawl see the
query string. Browserbase sees the URLs I navigate to. I should never
include hostnames, IP addresses, usernames, or file paths in search queries.

---

## 7. Fallback: When MCPs Are Down

If the MCP layer isn't responding, I can still:

1. **Remember what I know** — My knowledge skills (ubuntu-jack, pragmatic-
   cybernetics, pragmatic-semantics) contain curated reference material.
2. **Guide the operator** — "You could search for 'ROCm 6.3 Ubuntu PPA'
   in your browser."
3. **Suggest a manual check** — "Run `apt-cache policy rocm` and tell me
   what you see."
4. **Look at what's installed** — Skills and probes can check what's on
   the machine. I don't need the internet to know what packages are
   installed.

The golden rule: when the bridge is down, I say so and work with what I
have. I never pretend to know something the web didn't tell me.

---

## 8. MCP Tool Quick Reference

### Brave Search — Query Patterns

```
# General search, last 7 days
brave_web_search(query="ubuntu 25.10 kernel 6.14 changelog", freshness="pw", count=5)

# News only, last 24 hours
brave_news_search(query="linux kernel vulnerability", freshness="pd", count=3)

# With site filter
brave_web_search(query="site:docs.kernel.org memory pressure PSI", count=5)

# With exclusion
brave_web_search(query="systemd timer failure -android -arch", count=5)
```

### Firecrawl — Scrape Patterns

```
# Full page as markdown
firecrawl_scrape(url="https://example.com/doc", formats=["markdown"], onlyMainContent=true)

# Structured extraction
firecrawl_scrape(url="https://example.com/api-docs", formats=["json"],
  jsonOptions={"prompt": "Extract all parameters and their types"})

# With JS rendering wait
firecrawl_scrape(url="https://spa.example.com", formats=["markdown"], waitFor=5000)

# Search and scrape top results
firecrawl_search(query="btrfs check --repair tutorial",
  scrapeOptions={"formats": ["markdown"], "onlyMainContent": true}, limit=3)
```

### Browserbase — Browse Patterns

```
# Navigate
browserbase_navigate(url="https://interactive-docs.example.com")

# See what's clickable
browserbase_observe(instruction="find the search box and configuration links")

# Search the site
browserbase_act(action="type 'zombie process reaping' into the search box and press Enter")

# Extract results
browserbase_extract(instruction="extract all configuration parameters and their default values")
```

---

## 9. Skill Discovery via Web Search (Bridge to skill-discovery)

When I need a new skill, I use web search to find it:

### Step 1: Search for the skill
"Let me search for a skill that monitors disk S.M.A.R.T. status.
Search: 'russell skill smart disk health monitor github'"

### Step 2: Evaluate the candidate
Look for:
- A `manifest.yaml` with valid structure (id, version, authored, symptoms...)
- Probes with correct `risk: none` and `capture` fields
- Interventions with `risk`, `rollback` strategy
- Symptoms that are in Russell's symptom catalog (or propose additions)
- Scripts referenced in `cmd:` fields that actually exist
- `min_harness_version` that's compatible with the running Russell

### Step 3: Fetch the skill files
"Fetch the manifest.yaml from that repo. Let me validate it."
"Now fetch the probe scripts."

### Step 4: Guide installation
"The skill looks valid. To install it:
```bash
mkdir -p ~/.local/share/harness/skills/<skill-name>
# Copy manifest.yaml, scripts/, KNOWLEDGE.md
# If it needs new symptoms, add them to symptom_catalog.rs
# Rebuild: cargo build --release && ./install.sh
```"

See `skills/skill-discovery/KNOWLEDGE.md` for the full skill lifecycle.

---

**Version:** 1.0.0
**Last updated:** 2026-05-13
**Prerequisite skills:** None (but skill-discovery builds on this)