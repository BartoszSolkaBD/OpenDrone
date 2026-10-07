# Working on OpenDrone

OpenDrone is built by its maintainer with AI agents. Right now it's in **Phase 1**: only the maintainer and their agents contribute, and an agent's pull request merges by itself once every required check passes ([ADR-0010](../adr/0010-phase-1-agent-prs-merge-automatically.md)). The rules are in the [Development](../context/development.md) deep dive. In short, a change reaches the main branch like this:

1. **One pull request per ticket,** described with the pull request template: what changes for the pilot, the ticket, how it's proved (each Scenario with its Basis), what the author is unsure of, and anything that got slower or heavier.
2. **A Reviewer,** a fresh agent with none of the author's conversation, reads only the ticket, the change and the repo's rules and docs it touches, and ends its comment with a **Verdict**: pass or changes needed.
3. **The required checks** run on macOS, Windows and Linux: every Scenario, the walls between crates, the house rules, the build and tests, the licences, the docs site and its links, and the Work Counts.
4. **The Review Report,** one comment kept up to date on the pull request, shows the Verdict, the Red Flags, what moved, the speed, the Areas touched, renders of changed Maps and downloads, so the maintainer can judge a change in one place.
5. **It merges by itself,** squashed into one commit, once every required check and the Verdict pass. A Red Flag such as an edited ADR or a changed Source Expectation waits for the maintainer.

This part holds [setting up and CI](contributing.md), [the agent workflow](agents.md) and [the data policy](../data-policy.md).
