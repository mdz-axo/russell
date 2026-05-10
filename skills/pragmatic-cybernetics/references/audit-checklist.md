# Audit Checklist

> The complete 20-point Cybernetic Audit Checklist for systematic adversarial review of software systems, test harnesses, agent loops, and monitoring infrastructure.

---

## The Cybernetic Audit Checklist

For systematic adversarial review of any software system, test harness, or monitoring infrastructure:

### 1. REQUISITE VARIETY
> Does the controller have requisite variety to match the system's disturbances?

- How many distinct failure modes can the system exhibit?
- How many does the test harness cover?
- Are there entire categories of disturbance the regulator cannot represent?
- **Red flag**: A test suite with 50 tests for a system with 5000 states.

### 2. GOOD REGULATOR (Conant-Ashby)
> Does the test harness contain an accurate model of the system?

- Where does the harness's model diverge from actual behavior?
- Is the model updated when the system changes?
- Does it model failure modes, or only success modes?
- **Red flag**: A harness that only tests happy paths.

### 3. ALGEDONIC SIGNALS
> Do critical alerts bypass normal channels and escalate on timeout?

- Can algedonic signals be suppressed or ignored by intermediate layers?
- Is there a timeout/escalation mechanism?
- Are there pleasure signals (unexpected successes)?
- **Red flag**: Alert fatigue — the pain signal attenuated to zero by overuse.

### 4. FEEDBACK CLOSURE
> Does every output have a feedback path? Are there broken loops?

- Are there actions the system takes that are never observed or measured?
- What is the delay in each loop? Are delays causing oscillation?
- Are there loops that exist on paper but are not connected in implementation?
- **Red flag**: A system that writes but never reads back to verify.

### 5. VARIETY ATTENUATION
> Is filtering preserving signal while removing noise?

- At each interface, is signal being preserved or filtered out?
- Is information being lost that the regulator needs?
- **Red flag**: A logging system that captures everything but surfaces nothing useful.

### 6. VARIETY AMPLIFICATION
> Does the controller have enough response options?

- At each control point, does the regulator have sufficient response diversity?
- Can the regulator only say "up/down" when it needs 10 response modes?
- **Red flag**: A monitoring system that can only alert "up" or "down."

### 7. OBSERVER-SYSTEM COUPLING
> Does observation change what's being observed?

- Does the test harness alter system behavior by its presence?
- Are there behaviors that only manifest when the harness is absent?
- Does the system detect it's being tested and behave differently?
- **Red flag**: System passes all tests in staging but fails in production.

### 8. RECURSION
> Does the system model apply at every level?

- Does the VSM (S1–S5) hold at every level of recursion?
- Are there levels where the model breaks?
- Is the test harness itself a viable system with its own S1–S5?
- **Red flag**: Tests only at integration level, not at unit or system level.

### 9. ULTRASTABILITY
> Does the system have both inner (parameter) and outer (structure) loops?

- Can the system adjust its own parameters when behavior degrades?
- Can it restructure when parameter adjustment fails?
- **Red flag**: System can only correct errors, never question its own assumptions.

### 10. AUTOPOIESIS
> Does the system maintain its own identity under perturbation?

- Does the system preserve its essential invariants during adaptation?
- Can the system recover its identity after partial failure?
- **Red flag**: System adapts by abandoning its core contracts.

### 11. HOMEOSTASIS
> Do essential variables stay within viable ranges?

- What are the system's essential variables?
- What are their viable ranges?
- What mechanisms keep them within range?
- **Red flag**: No defined viable range for critical system variables.

### 12. DOUBLE-LOOP LEARNING
> Can the system question its own assumptions, not just correct errors?

- Does the system only do single-loop correction (fix the error)?
- Can it do double-loop questioning (change the goal/parameter)?
- Can it do triple-loop reflection (question how it questions)?
- **Red flag**: System fixes symptoms but never addresses root causes.

### 13. SYSTEM 3* INTEGRITY
> Does the sporadic audit actually bypass normal channels?

- Does the test harness read the system's own reporting, or independently probe?
- Does the audit function have independent access to operational reality?
- **Red flag**: Test harness reads the system's own health endpoint instead of independently verifying.

### 14. DISHONESTY DETECTION
> Does surface behavior accurately represent the computation performed?

- Would a user correctly understand what processing occurred?
- Are there mock passthroughs, simulated outputs, or silent fallbacks?
- **Red flag**: Observable behavior misrepresents actual computational work.

### 15. AGENT PROGRESS
> Does the agent have a progress metric that is structurally distinct from activity?

- Can you distinguish "busy and making progress" from "busy and stuck"?
- Is there a defined convergence criterion for the agent's current goal?
- Does the system detect progress flatline and trigger escalation?
- **Red flag**: Agent executes hundreds of tool calls with no measurable goal convergence.

### 16. COGNITIVE READINESS
> Can the agent assess its own ability to reason correctly about the current task?

- Does the agent have signals for context saturation, uncertainty, or confusion?
- Can it recognize when its model of the situation is likely wrong?
- Is there a mechanism to pause and request clarification rather than hallucinate forward?
- **Red flag**: Agent confidently produces incorrect outputs with no self-doubt signal.

### 17. SKILL TRUST BOUNDARY
> Are skill capabilities properly attenuated by trust tier? Can a skill escalate its own trust?

- Is each skill's operational variety bounded by its trust tier?
- Are there mechanisms preventing a skill from expanding its own permissions?
- Is trust demotion faster than trust escalation (Slovic asymmetry)?
- **Red flag**: A T2 skill can trigger operations that should require T4.

### 18. COORDINATION ANTI-OSCILLATION
> In multi-agent systems, do coordination mechanisms prevent thundering herd and oscillatory behavior?

- Is there an S2 function that dampens oscillation between competing agents?
- Do backoff strategies prevent restart loops and resource contention?
- Can the system detect and break deadlock between agents?
- **Red flag**: Multiple agents simultaneously retry the same failed operation.

### 19. CONTEXT WINDOW AS CHANNEL CAPACITY
> Is the finite context window treated as an information channel with bounded capacity (Shannon)?

- Is context consumption monitored and budgeted?
- Does the system attenuate variety (summarize, compress, discard) before context overflow?
- Are there mechanisms to preserve high-value information and discard low-value tokens?
- **Red flag**: Agent loses critical context because low-priority information filled the window.

### 20. RECURSIVE MONITORING INDEPENDENCE
> Does the monitoring system avoid consuming the same resources it monitors?

- Is the monitoring path architecturally independent from the operational path?
- Can the monitor detect its own failure (watchdog)?
- Does monitoring instrumentation avoid degrading the system it observes?
- **Red flag**: Monitoring system goes down in the same outage it should have detected.
