# Discourse Framework

Turn, SpeechAct, DiscourseRelation, EpistemicLevel, and AcquisitionMethod — the conversational discourse representation types.

---

## Conversational Discourse Framework

**Implementation:** First-class discourse representation following Austin (1962), Searle (1969), Mann & Thompson (1988), Clark (1996).

**Core types:**

**`Turn`** — atomic unit of conversation:
- `turn_id: TurnId`, `session_id: SessionId`, `seq: u64` (monotonic)
- `speaker: ParticipantId`, `timestamp: DateTime<Utc>`, `content: String`
- Semantic annotations (progressive, all `Option`): `speech_act`, `point`, `epistemic`, `mood`
- Pragmatic context: `presuppositions: Vec<Presupposition>`, `implicatures: Vec<Implicature>`, `references: Vec<DiscourseReference>`
- Discourse relations: `relations: Vec<DiscourseRelation>`
- System integration: `extracted_goals`, `extracted_constraints`, `produced_facts`

**`SpeechAct`** — Austin/Searle taxonomy:

| Variant | Semantic Mapping | Example |
|---------|-------------|---------|
| `Assertive` | Produces Facts | "The token doesn't expire" |
| `Directive` | Produces Goals | "Fix the auth module" |
| `Commissive` | Produces Contract terms | "I'll handle the refactoring" |
| `Expressive` | Personality (no Goal) | "Thanks for explaining that" |
| `Declarative` | Modifies ontology | "Let's call this a 'sprint'" |

**`RelationKind`** — discourse relations — 11 variants:
`RespondsTo`, `Elaborates`, `Corrects`, `Challenges`, `Acknowledges`, `Clarifies`, `RequestsClarification`, `Summarizes`, `Meta`, `Presupposes`, `Retracts`

**`EpistemicLevel`** — confidence mapping:
- `Certain` → Confidence 0.95
- `Probable` → Confidence 0.75
- `Possible` → Confidence 0.55
- `Hypothetical` → Confidence 0.3
