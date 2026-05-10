# Mermaid Diagram Types — Complete Reference

## Table of Contents

1. [Gantt Chart](#gantt-chart)
2. [Pie Chart](#pie-chart)
3. [Git Graph](#git-graph)
4. [Mindmap](#mindmap)
5. [Timeline](#timeline)
6. [Quadrant Chart](#quadrant-chart)
7. [Sankey Diagram](#sankey-diagram)
8. [XY Chart](#xy-chart)
9. [Block Diagram](#block-diagram)
10. [Packet Diagram](#packet-diagram)
11. [Architecture Diagram](#architecture-diagram)
12. [Kanban Board](#kanban-board)
13. [C4 Diagram](#c4-diagram)
14. [User Journey](#user-journey)

Core diagram types (Flowchart, Sequence, Class, State, ER) are documented in SKILL.md.

---

## Gantt Chart

```
gantt
    title Project Schedule
    dateFormat  YYYY-MM-DD
    axisFormat  %b %d
    
    section Design
        Requirements    :done,    des1, 2024-01-01, 2024-01-14
        Architecture    :active,  des2, 2024-01-15, 30d
        Review          :         des3, after des2, 7d
    
    section Implementation
        Core Module     :         impl1, after des3, 45d
        Testing         :         impl2, after impl1, 21d
        Milestone       :milestone, m1, after impl2, 0d
    
    section Deployment
        Staging         :         dep1, after m1, 7d
        Production      :crit,    dep2, after dep1, 3d
```

**Task modifiers**: `done` (completed), `active` (in progress), `crit` (critical path), `milestone` (zero-duration marker).

**Duration**: `30d` (days), `after taskId` (dependency), or explicit date range.

> **Formal model**: Interval scheduling with precedence constraints. Tasks are intervals on a timeline; `after` defines a partial order. Critical path analysis identifies the longest dependency chain.

---

## Pie Chart

```
pie title Distribution of Languages
    "Rust" : 45
    "Haskell" : 25
    "Python" : 20
    "TypeScript" : 10
```

> **Formal model**: Partition of unity. Each slice represents a proportion p_i where sum(p_i) = total. Rendered as circular sectors with arc length proportional to value.

---

## Git Graph

```
gitGraph
    commit id: "init"
    branch develop
    checkout develop
    commit id: "feature-1"
    commit id: "feature-2"
    checkout main
    merge develop id: "merge-1" tag: "v1.0"
    commit id: "hotfix"
    branch release
    checkout release
    commit id: "rc-1"
    checkout main
    merge release id: "release" tag: "v1.1"
```

**Commands**: `commit`, `branch`, `checkout`, `merge`, `cherry-pick`.

**Modifiers**: `id:` (commit label), `tag:` (version tag), `type: NORMAL|REVERSE|HIGHLIGHT`.

> **Formal model**: Directed Acyclic Graph (DAG) where nodes are commits and edges point to parent commits. Branches are named pointers. Merge creates a node with multiple parents.

---

## Mindmap

```
mindmap
    root((Central Idea))
        Topic A
            Subtopic A1
            Subtopic A2
        Topic B
            Subtopic B1
                Detail B1a
                Detail B1b
        Topic C
```

**Node shapes**: `((text))` circle, `(text)` rounded rectangle, `[text]` rectangle, `{{text}}` hexagon, `)text(` cloud, default plain text.

> **Formal model**: Rooted tree T = (V, E, r) where r is the root. Children are determined by indentation level. No cycles permitted.

---

## Timeline

```
timeline
    title History of Web Frameworks
    2004 : Ruby on Rails
    2005 : Django
    2010 : Express.js
         : AngularJS
    2013 : React
    2014 : Vue.js
    2016 : Angular 2
    2020 : Next.js
         : Svelte Kit
```

> **Formal model**: Totally ordered set of epochs, each containing one or more events. Events within an epoch are unordered (simultaneous).

---

## Quadrant Chart

```
quadrant-beta
    title Technology Assessment
    x-axis Low Complexity --> High Complexity
    y-axis Low Value --> High Value
    quadrant-1 Invest Heavily
    quadrant-2 Maintain
    quadrant-3 Eliminate
    quadrant-4 Evaluate
    Mermaid: [0.7, 0.8]
    D3.js: [0.9, 0.9]
    MS Paint: [0.1, 0.1]
    PowerPoint: [0.3, 0.5]
```

> **Formal model**: Cartesian plane partitioned into four quadrants by two orthogonal axes. Items are points (x, y) in [0, 1] x [0, 1].

---

## Sankey Diagram

```
sankey-beta
    Source A, Target X, 25
    Source A, Target Y, 15
    Source B, Target X, 10
    Source B, Target Z, 20
    Target X, Final, 35
    Target Y, Final, 15
    Target Z, Final, 20
```

> **Formal model**: Weighted directed graph with flow conservation: for each intermediate node, sum of incoming flows = sum of outgoing flows. Width of edges is proportional to flow magnitude.

---

## XY Chart

```
xychart-beta
    title "Monthly Revenue"
    x-axis [Jan, Feb, Mar, Apr, May, Jun]
    y-axis "Revenue ($K)" 0 --> 150
    bar [50, 60, 75, 90, 100, 130]
    line [50, 60, 75, 90, 100, 130]
```

> **Formal model**: Cartesian coordinate system with discrete or continuous x-axis. Bar and line series map data points to visual marks.

---

## Block Diagram

```
block-beta
    columns 3
    
    Frontend:3
    
    block:backend:2
        API["API Gateway"]
        Auth["Auth Service"]
    end
    
    DB[("Database")]:1
    
    Frontend --> API
    API --> Auth
    API --> DB
```

**Layout**: `columns N` defines the grid. Blocks span columns with `:N` suffix. Nested blocks with `block:id:span`.

> **Formal model**: Nested component graph with grid-based spatial layout. Components can contain sub-components.

---

## Packet Diagram

```
packet-beta
    0-7: "Version"
    8-15: "Traffic Class"
    16-19: "Flow Label (high)"
    20-31: "Flow Label (low)"
    32-47: "Payload Length"
    48-55: "Next Header"
    56-63: "Hop Limit"
```

> **Formal model**: Bit field layout. Each field occupies a contiguous range of bit positions. Total width is typically 32 bits per row (network protocol convention).

---

## Architecture Diagram

```
architecture-beta
    group cloud(cloud)[Cloud]
    
    service api(server)[API Server] in cloud
    service db(database)[PostgreSQL] in cloud
    service cache(database)[Redis] in cloud
    
    api:R --> L:db
    api:B --> T:cache
```

**Elements**: `group` (container), `service` (component with icon), directional edges with port positions (L/R/T/B).

> **Formal model**: Component graph with spatial grouping and directional port connections. Based on the C4 component model abstraction.

---

## Kanban Board

```
kanban
    column1[Todo]
        task1[Design API]
        task2[Write tests]
    column2[In Progress]
        task3[Implement core]
    column3[Done]
        task4[Setup CI]
```

> **Formal model**: Column-partitioned set where each column represents a workflow stage. Tasks are elements assigned to exactly one column.

---

## C4 Diagram

```
C4Context
    title System Context Diagram
    
    Person(user, "User", "Uses the system")
    System(system, "Discourse Agent", "Cognitive reasoning engine")
    System_Ext(llm, "LLM Provider", "OpenRouter/Ollama")
    System_Ext(db, "Memory Store", "sled database")
    
    Rel(user, system, "Sends queries")
    Rel(system, llm, "Calls for reasoning")
    Rel(system, db, "Reads/writes facts")
```

**Levels**: `C4Context` (system context), `C4Container` (container), `C4Component` (component), `C4Dynamic` (runtime).

**Elements**: `Person`, `System`, `System_Ext`, `Container`, `Component`, `Rel`, `BiRel`.

> **Formal model**: C4 model (Simon Brown, 2011). Four hierarchical abstraction levels: Context (systems + people), Container (deployable units), Component (modules within containers), Code (classes/functions).[^c4]

[^c4]: Brown, S. (2011). *The C4 Model for Software Architecture*. [c4model.com](https://c4model.com/)

---

## User Journey

```
journey
    title User Onboarding
    section Registration
        Visit website: 5: User
        Fill form: 3: User
        Verify email: 2: User, System
    section First Use
        Complete tutorial: 4: User, System
        Create first project: 5: User
    section Retention
        Return next day: 3: User
        Invite colleague: 4: User
```

**Format**: `Task name: satisfaction_score: actors`. Score is 1-5 (1=frustrated, 5=delighted).

> **Formal model**: Scored timeline partitioned into sections. Each task has a satisfaction metric and actor assignment, enabling experience mapping analysis.

---

## Formal Model Summary

| Diagram | Formal Model | Key Properties |
|---------|-------------|---------------|
| Flowchart | Directed graph G=(V,E) | Nodes, edges, subgraph hierarchy |
| Sequence | Message Sequence Chart | Lifelines, partial event order, combined fragments |
| Class | UML class metamodel | Inheritance, composition, aggregation, multiplicity |
| State | Finite automaton (Q,Σ,δ,q₀,F) | States, transitions, nesting (Harel) |
| ER | Entity-Relationship model | Entities, relationships, cardinality constraints |
| Gantt | Interval scheduling | Durations, precedence constraints, critical path |
| Pie | Partition of unity | Proportional slices summing to total |
| Git Graph | DAG | Commits, branches (named pointers), merges |
| Mindmap | Rooted tree | Hierarchy by indentation, single root |
| Timeline | Totally ordered set | Epochs containing events |
| Quadrant | Cartesian quadrant partition | Points in [0,1]x[0,1], four labeled regions |
| Sankey | Weighted directed graph | Flow conservation at nodes |
| XY Chart | Cartesian coordinate plot | Discrete/continuous axes, bar/line series |
| Block | Nested component graph | Grid layout, containment hierarchy |
| Packet | Bit field layout | Contiguous bit ranges, row-based display |
| Architecture | Component graph with ports | Groups, services, directional connections |
| Kanban | Column-partitioned set | Workflow stages, task assignment |
| C4 | C4 hierarchical model | 4 abstraction levels (Brown, 2011) |
| Journey | Scored timeline | Tasks with satisfaction metrics and actors |

---

## References

- Mermaid documentation: [mermaid.js.org](https://mermaid.js.org/)
- Chen, P. (1976). *The Entity-Relationship Model*. ACM TODS.
- Harel, D. (1987). *Statecharts*. Science of Computer Programming.
- Sugiyama, K. et al. (1981). *Methods for Visual Understanding of Hierarchical System Structures*. IEEE Trans.
- Brown, S. (2011). *The C4 Model*. [c4model.com](https://c4model.com/)
