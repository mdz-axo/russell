---
name: pragmatic-cybernetics
description: Pragmatic cybernetics skill grounded in Norbert Wiener's foundational work (1948), the Macy Conferences (1946–1953), Ashby's Law of Requisite Variety (1956), Beer's Viable System Model (1972), second-order cybernetics (von Foerster, Maturana, Varela), and Argyris's double-loop learning. Extends classical cybernetics into the modern AI agent ecosystem — covering OODA/MAPE-K agent control loops, multi-agent coordination, self-healing resilience patterns, skill trust lifecycle governance, OKH telemetric architecture, and agent observability as cybernetic feedback. Provides tools and perspectives for analyzing feedback loops, variety engineering, homeostatic regulation, algedonic signals, observer-system coupling, and recursive viability in software systems. Primary use cases: adversarial review of test harnesses (System 3* audit), trust dynamics analysis, self-healing architecture design, agentic system governance, OKH instrumentation, and cybernetic evaluation of monitoring and control systems. Includes the 20-point Cybernetic Audit Checklist for systematic adversarial review.
license: MIT
metadata:
  skill-author: Pragmatic Cybernetics Contributors
  version: "3.0.0"
  tags:
    - cybernetics
    - systems-theory
    - feedback-loops
    - viable-system-model
    - adversarial-review
    - test-harness-design
    - variety-engineering
    - trust-dynamics
    - second-order-cybernetics
    - homeostasis
    - agentic-systems
    - ooda-loop
    - mape-k
    - self-healing
    - agent-observability
    - skill-trust-governance
    - okh-telemetry
    - multi-agent-coordination
  sources:
    - "Wiener, N. (1948). Cybernetics: Or Control and Communication in the Animal and the Machine. MIT Press."
    - "Ashby, W.R. (1956). An Introduction to Cybernetics. Chapman & Hall."
    - "Ashby, W.R. (1952). Design for a Brain. Chapman & Hall."
    - "Beer, S. (1972). Brain of the Firm. Allen Lane / Penguin Press."
    - "Beer, S. (1979). The Heart of Enterprise. John Wiley & Sons."
    - "Beer, S. (1985). Diagnosing the System for Organisations. John Wiley & Sons."
    - "Conant, R.C. & Ashby, W.R. (1970). Every Good Regulator of a System Must Be a Model of That System. International Journal of Systems Science, 1(2), 89–97."
    - "von Foerster, H. (1974). Cybernetics of Cybernetics. BCL Report 73.38. University of Illinois."
    - "Maturana, H.R. & Varela, F.J. (1980). Autopoiesis and Cognition: The Realization of the Living. D. Reidel."
    - "Argyris, C. (1977). Double Loop Learning in Organizations. Harvard Business Review, 55(5), 115–125."
    - "Rosenblueth, A., Wiener, N., & Bigelow, J. (1943). Behavior, Purpose and Teleology. Philosophy of Science, 10(1), 18–24."
    - "McCulloch, W.S. & Pitts, W. (1943). A Logical Calculus of the Ideas Immanent in Nervous Activity. Bulletin of Mathematical Biophysics, 5, 115–133."
    - "Shannon, C.E. (1948). A Mathematical Theory of Communication. Bell System Technical Journal, 27, 379–423, 623–656."
    - "Slovic, P. (1993). Perceived Risk, Trust, and Democracy. Risk Analysis, 13(6), 675–682."
    - "Wilson, J.Q. & Kelling, G.L. (1982). Broken Windows. The Atlantic Monthly, 249(3), 29–38."
    - "Boyd, J.R. (1987). A Discourse on Winning and Losing. Unpublished briefing."
    - "Kephart, J.O. & Chess, D.M. (2003). The Vision of Autonomic Computing. IEEE Computer, 36(1), 41–50."
    - "Armstrong, J. (2003). Making Reliable Distributed Systems in the Presence of Software Errors. PhD Thesis, KTH."
---

# Pragmatic Cybernetics

## Overview

Cybernetics is the science of **control and communication in the animal and the machine** (Wiener 1948). This skill provides pragmatic tools for analyzing, designing, and adversarially reviewing software systems through a cybernetic lens. Every concept maps to a concrete engineering practice.

The central insight: **a software system is a cybernetic system** — with sensors (observability), actuators (deployments), feedback loops (metrics → alerts → responses), regulators (test harnesses, circuit breakers), and a nervous system (tracing, monitoring). Understanding these as cybernetic structures reveals architectural blind spots that conventional analysis misses.

## When to Use This Skill

- **Adversarial review** of test harnesses, monitoring systems, or observability pipelines
- **Analyzing feedback loops** in CI/CD, deployment, alerting, or self-healing architectures
- **Evaluating requisite variety** — does the regulator have enough response diversity to match the system's failure modes?
- **Designing trust dynamics** — how trust is built, destroyed, and regulated in multi-component systems
- **Assessing organizational viability** using the Viable System Model (VSM)
- **Detecting broken feedback loops** — signals emitted but never consumed, or consumed but never acted upon
- **Second-order analysis** — evaluating whether the monitoring system itself is healthy (monitoring the monitor)

## When NOT to Use This Skill

- Pure algorithmic optimization (use mathematical optimization tools)
- Low-level performance profiling (use profilers and benchmarks)
- Syntax or grammar questions about specific languages
- Simple CRUD application design where feedback complexity is minimal

## Quick Reference

| Concept | One-Line Definition | Engineering Application |
|---------|--------------------|-----------------------|
| Feedback loop | Output fed back as input for self-correction | Metrics → alerts → auto-remediation |
| Negative feedback | Counteracts deviation from setpoint | Circuit breakers, rate limiters, auto-scaling |
| Positive feedback | Amplifies deviation | Viral growth, recommendation amplification, cascading failures |
| Requisite variety | Controller variety ≥ system disturbance variety | Test coverage ≥ failure mode count |
| Homeostasis | Self-regulation to maintain essential variables in viable range | SLO enforcement, health checks |
| Algedonic signal | Pain/pleasure signal bypassing normal channels | PagerDuty, circuit breakers, automated rollback |
| Variety attenuation | Filtering high-variety signals to manageable volume | Log aggregation, metric rollups, dashboards |
| Variety amplification | Expanding controller response repertoire | Policy frameworks, delegation, automated responses |
| Ultrastability | Double feedback: inner loop adjusts behavior, outer loop adjusts parameters | Feature flags + A/B testing, blue-green deploys |
| Autopoiesis | Self-producing system maintaining its own identity | Self-healing infrastructure, identity-preserving migrations |
| System 3* | Sporadic direct audit bypassing normal reporting | Test harnesses, chaos engineering, penetration testing |
| Good regulator | Every good regulator must be a model of its system | Test harness must model the system's actual behavior |

## Anti-Patterns

| Anti-Pattern | Cybernetic Violation | Consequence |
|-------------|---------------------|-------------|
| **Alert fatigue** | Broken algedonic channel | Pain signals attenuated to zero; real emergencies missed |
| **Dashboard theater** | Variety attenuation without amplification | Information displayed but never acted upon |
| **Test-only happy paths** | Incomplete regulator model | System appears healthy; failures go undetected |
| **Monitoring the monitor's output** | System 3 reading System 3's own reports instead of S3* | Model verifies itself; no ground truth check |
| **Single-loop-only fixes** | Missing ultrastability | Symptoms addressed; root causes persist |
| **Coverage as goal** | Goodhart's Law (related to requisite variety) | Test count maximized; behavioral coverage minimized |
| **Production ≠ staging** | Observer-system coupling failure | Tests pass in observed environment, fail in unobserved one |
| **Trust without verification** | Missing System 3* | Trust assumed, never audited; silent degradation |

---

## Routing Decision Tree

Use this to determine which reference file to load for the current task:

```
What is the task?
├── Historical context, foundational theory, VSM details?
│   └── → references/classical-foundations.md
├── Applying cybernetic methods to a specific system?
│   └── → references/methods-and-tools.md
├── Running the adversarial audit checklist?
│   └── → references/audit-checklist.md
├── Analyzing/designing agent loops, self-healing, supervision?
│   └── → references/agentic-systems.md
├── Evaluating skill trust, progressive disclosure, governance?
│   └── → references/skill-governance.md
└── OKH instrumentation, agent observability, health metrics?
    └── → references/okh-observability.md
```

## Linked Reference Files

| File | Read When | Contents |
|------|-----------|----------|
| `references/classical-foundations.md` | Need historical context, Ashby's Law details, VSM system descriptions, second-order cybernetics theory | Wartime origins, Macy Conferences, first-order cybernetics (Requisite Variety, Homeostat, Black Box), Beer's full VSM (S1–S5 + S3*), algedonic signals, recursion principle, variety engineering, second-order cybernetics (autopoiesis, constructivism), primary source bibliography |
| `references/methods-and-tools.md` | Applying cybernetic analysis to a concrete system, need diagrams or decision trees | Feedback loop analysis, variety analysis, double-loop learning, circular causality, Good Regulator analysis, observability mapping, test harness audit questions, Mermaid diagrams (VSM, feedback loop, test architecture, double-loop), decision trees |
| `references/audit-checklist.md` | Running the full 20-point adversarial audit | All 20 checklist items with probing questions and red flags: Requisite Variety, Good Regulator, Algedonic Signals, Feedback Closure, Variety Attenuation/Amplification, Observer Coupling, Recursion, Ultrastability, Autopoiesis, Homeostasis, Double-Loop Learning, S3* Integrity, Dishonesty Detection, Agent Progress, Cognitive Readiness, Skill Trust Boundary, Coordination Anti-Oscillation, Context Window, Recursive Monitoring Independence |
| `references/agentic-systems.md` | Analyzing agent loops, diagnosing agent pathologies, designing self-healing or multi-agent coordination | OODA/MAPE-K agent loops, VSM-to-agent-architecture mapping, cognitive maneuverability, agentic pathologies table, self-healing closed loop, Erlang/OTP patterns, three-tier escalation, anti-oscillation, stuck detection, durable execution, monitoring independence |
| `references/skill-governance.md` | Evaluating skill trust tiers, progressive disclosure design, supply-chain security | Four-tier trust model (T1–T4), progressive disclosure as variety attenuation, trust escalation gates (G1–G4), Slovic asymmetry in demotion, supply-chain attacks as variety injection, RBAC as variety engineering |
| `references/okh-observability.md` | Instrumenting OKH spans, designing agent health metrics, building observability for agent fleets | OKH namespace-to-VSM mapping, observation→control loop diagram, OpenTelemetry GenAI conventions, agent health metrics (cognitive readiness, progress rate, reasoning quality), distributed tracing across agents, fleet-level attenuation, AI-enhanced dashboards as amplification, agents-monitoring-agents, observability signals quick reference |
