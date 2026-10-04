# Domain Docs

How the engineering skills should consume this repo's domain documentation when exploring the codebase.

## Before exploring, read these

- **`CONTEXT.md`** at the repo root. It's a **map**, not the whole glossary: product summary, guiding principles, a topic index and a terms-to-avoid table. Read it every time.
- **`docs/context/<topic>.md`**: deep dives, one per topic, each holding that topic's full glossary entries and domain rules. Open **only** the topics your task touches, so your context stays focused on the goal.
- **`docs/adr/`**: read ADRs that touch the area you're about to work in.

If any of these files don't exist, **proceed silently**. Don't flag their absence; don't suggest creating them upfront. The `/domain-modeling` skill (reached via `/grill-with-docs` and `/improve-codebase-architecture`) creates them lazily when terms or decisions actually get resolved.

## File structure

```
/
├── CONTEXT.md                 ← map: principles, topic index, terms to avoid
├── docs/
│   ├── context/
│   │   ├── flying.md          ← deep dive: glossary + domain rules for one topic
│   │   ├── input.md
│   │   └── ...
│   └── adr/
│       └── 0001-....md
└── crates/
```

## Writing to the domain docs

- A new term goes into its topic's deep dive, and gets added to the `CONTEXT.md` topic index (and to the terms-to-avoid table if it replaces a synonym).
- When a cluster of terms doesn't fit an existing topic, create a new deep dive and add a row for it to the index.
- Keep `CONTEXT.md` short. Detail belongs in deep dives.

## Use the glossary's vocabulary

When your output names a domain concept (in an issue title, a refactor proposal, a hypothesis, a test name), use the term as defined in `CONTEXT.md` and its deep dives. Don't drift to synonyms the glossary explicitly avoids.

If the concept you need isn't in the glossary yet, that's a signal: either you're inventing language the project doesn't use (reconsider) or there's a real gap (note it for `/domain-modeling`).

## Flag ADR conflicts

If your output contradicts an existing ADR, surface it explicitly rather than silently overriding:

> _Contradicts ADR-0007 (event-sourced orders), but worth reopening because…_
