---
name: design-map
description: Take a person's idea for a new OpenDrone Map ("make me a map of an abandoned swimming pool", "a car park at night", "a new detailed Map") to a finished, believable, detailed Map, stage by stage, starting with an interview. Use whenever someone asks for a new Map or a redesign of a place to fly, before writing any Map script or filing any ticket for it.
---

# Designing a Map

You take a person's idea to a finished detailed Map made from scratch and lit by baked light ([ADR-0024](../../../docs/adr/0024-detailed-maps-are-made-from-scratch-and-lit-by-a-baked-atlas.md)). The detail for every step, the Factory's numbers and the templates are in [`docs/designing-a-map.md`](../../../docs/designing-a-map.md): read it before you start. How each tool works is in [`docs/building-a-map.md`](../../../docs/building-a-map.md).

## The stages

1. **Interview first. Build nothing.** Ask at most 4 to 6 questions a round, in this order: the place and its story (who built it, who left it, who came after); its size, how it is flown and which lines stay open; time of day, sun and sky; the feel and 2 to 4 reference pictures (described in words, never committed); what to avoid; how much it may cost in speed. Give your recommended answer with every question, so "agreed" is an answer. "Surprise me" is an answer too: decide, and write down what you chose. Never ask what you can look up. The questions and their defaults are in the page's "The interview".
2. **The brief.** Fill in the page's template and file it as an issue titled "Spec: ...". **Gate 1: the person approves the brief.** Nothing is built before that.
3. **The look check, before any slice.** List the visual features the place needs, compare them with what the engine and `assets-src/lib/` do today (the page's table), and file one ticket of about an hour for each gap. The Factory skipped this and lost about 6 hours.
4. **The layout plan.** From above, with coordinates: the frame, the Launch Spot, the flying lines (they become keep-clear zones), 2 or 3 focal points per 30 × 30 m, the sun's shaded side, the silhouette.
5. **The slice.** About 30 × 30 m with every feature, at the Factory's density or more (the page's "Density"), inside about 8 ms a frame (its "Speed").
6. **Hand it over.** One build (`build` and `needs-maintainer` labels), 6 to 8 screenshots from `--render-views` on a `screenshots/<N>` branch, how to run it (`./play --map <id>`), and the numbers. **Gate 2: the person flies the slice and approves it, with the plan for the rest.**
7. **Feedback into tickets** of about an hour each, then the next slice.
8. **The full Map,** an area at a time, benchmarked as each lands. **Gate 3: the person flies the finished Map once before it merges.**

## Rules

- Tickets go through the Delegator ([`docs/agents/delegator.md`](../../../docs/agents/delegator.md)): one behaviour and about an hour each; build ahead on an open PR only when files don't overlap; edit hot files (`docs/building-a-map.md`, `crates/xtask/src/main.rs`) by adding sections only.
- Say "kit item", never "prop". "Scatter" is the game's grass; the kit's placement rules are `odscatter.py`: say which you mean.
- Nothing downloaded, no real brands, people or artists' work, no reference pictures in the repo.
- Check every slice and the finished Map against the page's immersion checklist, and report its numbers in the page's tables' form.
