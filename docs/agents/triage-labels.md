# Triage Labels

The skills speak in terms of five canonical triage roles. This file maps those roles to the actual label strings used in this repo's issue tracker.

| Label in mattpocock/skills | Label in our tracker | Meaning                                  |
| -------------------------- | -------------------- | ---------------------------------------- |
| `needs-triage`             | `needs-triage`       | Maintainer needs to evaluate this issue  |
| `needs-info`               | `needs-info`         | Waiting on reporter for more information |
| `ready-for-agent`          | `ready-for-agent`    | Fully specified, ready for an AFK agent  |
| `ready-for-human`          | `ready-for-human`    | Requires human implementation            |
| `wontfix`                  | `wontfix`            | Will not be actioned                     |

When a skill mentions a role (e.g. "apply the AFK-ready triage label"), use the corresponding label string from this table.

Every `ready-for-agent` ticket also carries a Lane label, `lane: <Lane>`, for each Lane it takes: `lane: Flight`, `lane: Game`, `lane: Input`, `lane: Sound`, `lane: Blackbox`, `lane: Content`, `lane: Repo rules` or `lane: Docs`. Whoever files the ticket adds them, by the files its "What to build" will change: see [The Delegator](delegator.md#lanes).

On a pull request, CI adds `needs-triage` after five failed review rounds, and takes it off once a `Triaged to #<issue>` comment counts: see [The Reviewer and the Verdict](reviewer.md#starting-a-reviewer).

Edit the right-hand column to match whatever vocabulary you actually use.
