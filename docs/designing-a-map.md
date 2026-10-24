# Designing a Map

This page takes an agent from a person's idea ("an abandoned swimming pool", "a car park at night") to a finished, believable, detailed Map. It is written from what the **Factory** effort did on 23 and 24 October 2026 ([spec #290](https://github.com/BartoszSolkaBD/OpenDrone/issues/290)): what worked, what cost time, and the numbers the Factory reached. [Building a Map](building-a-map.md) says how each tool works; this page says in what order to use them, what to ask the person, and how good is good enough.

The rules it rests on are in [ADR-0024](adr/0024-detailed-maps-are-made-from-scratch-and-lit-by-a-baked-atlas.md): a detailed Map is made from scratch by scripts, nothing is downloaded, and all its light is baked. The terms are in [World and content](context/world.md) and [Development](context/development.md). An agent asked to make a Map uses the skill `design-map` ([`.claude/skills/design-map/SKILL.md`](../.claude/skills/design-map/SKILL.md)), which walks through this page.

**Words.** The **person** is whoever asked for the Map; today that is the maintainer. A **kit item** is one loose thing from the kit ([World and content](context/world.md)). A **story object** is a thing placed by hand because the place's history put it there: a fire pit, a bed frame in the ash, a crashed whoop in a window grid. **Scatter** means the game's grass ([Grass](building-a-map.md#grass)); the kit's placement rules live in `odscatter.py` and are called "the kit's rules" here, so the two don't mix.

## If you are an agent starting a Map

1. **Build nothing yet.** Start the [interview](#1-the-interview): at most 4 to 6 questions a round, each with your recommended answer, so the person can answer "agreed". "Surprise me" is a full answer: decide, and write down what you chose.
2. **Never ask what you can look up:** which walls the sun shades, what the engine can draw, the Gap sizes, the Launch Spot rules.
3. **Write the [design brief](#2-the-brief)** from the template and file it as a spec issue. **Gate 1:** the person approves it. Nothing is built before that.
4. **Do the [look check](#3-the-look-check)** before any slice: list every visual feature the place needs, compare it with what the engine and [`assets-src/lib/`](../assets-src/lib/) do today, and file one ticket of about an hour for each gap. The Factory skipped this and paid about 6 hours of rework.
5. **Draw the [layout plan](#4-the-layout-plan)** from above: the site, the flying lines, the focal points, where the sun comes from.
6. **Build one [slice](#5-the-slice)** of about 30 × 30 m that carries every feature, at the [Factory's density](#density) or more, inside the [frame budget](#speed).
7. **[Hand it over](#6-handing-a-slice-over):** one build, 6 to 8 screenshots, how to run it. **Gate 2:** the person flies it and approves it, and the plan for the rest.
8. **Turn the [feedback](#7-turning-feedback-into-tickets)** into tickets of about an hour. Repeat the slice until it passes.
9. **[Grow it](#8-growing-it-into-the-full-map)** into the full Map, an area at a time, and [benchmark](#9-the-benchmark) each one. **Gate 3:** the person flies the finished Map once before it merges.
10. **Run it [as tickets](#running-it-as-tickets)** through the [Delegator](agents/delegator.md), with the [immersion checklist](#the-immersion-checklist) as the bar.

## The stages at a glance

| Stage | What the agent makes | Who approves | The Factory |
|---|---|---|---|
| Interview | 2 or 3 rounds of questions with recommended answers | the person answers | a grilling with the maintainer on 23 October 2026 |
| Brief | a one-page spec issue | **Gate 1:** the person | [#290](https://github.com/BartoszSolkaBD/OpenDrone/issues/290) and [ADR-0024](adr/0024-detailed-maps-are-made-from-scratch-and-lit-by-a-baked-atlas.md) |
| Look check | one ticket per missing feature, before the slice | the Delegator runs them | skipped for slice 1; done after it as [#315](https://github.com/BartoszSolkaBD/OpenDrone/issues/315) and [#318](https://github.com/BartoszSolkaBD/OpenDrone/issues/318) to [#323](https://github.com/BartoszSolkaBD/OpenDrone/issues/323), then [#338](https://github.com/BartoszSolkaBD/OpenDrone/issues/338) to [#340](https://github.com/BartoszSolkaBD/OpenDrone/issues/340) |
| Layout plan | a plan from above with coordinates | shown with the slice | the hall corner's frame in [the research](research/lived-in-places.md#6-the-factorys-story) |
| Slice | about 30 × 30 m with every feature | **Gate 2:** the person flies it | three slices: [PR #317](https://github.com/BartoszSolkaBD/OpenDrone/pull/317), [PR #335](https://github.com/BartoszSolkaBD/OpenDrone/pull/335) and the third on `ticket/341-factory-slice-v3` |
| Full Map | the whole site, an area at a time | **Gate 3:** the person flies it once | not reached yet |

## 1. The interview

The interview turns an idea into answers the brief needs. Its rules:

- **At most 4 to 6 questions a round,** in the order below. Two or three rounds are enough.
- **Every question comes with your recommended answer,** worked out for this place, so the person can reply "agreed", or "agreed except 3". Start from the defaults below and make them fit the idea.
- **Never ask what you can look up.** The sun's side decides which walls are shaded and damp; the World rules decide the Gap sizes and the Launch Spot; this page and [Building a Map](building-a-map.md) say what the engine draws.
- **"Surprise me" is a valid answer,** to one question or to all of them. Then you decide, and the brief lists every choice you made under "Chosen by the agent", so the person can overrule any of them at Gate 1.
- **Reference pictures stay out of the repo.** The person's own photos are welcome as a source ([ADR-0024](adr/0024-detailed-maps-are-made-from-scratch-and-lit-by-a-baked-atlas.md)), but only your words about them go in the brief and the tickets, never the pictures. Describe each one in a sentence or two, as [#341](https://github.com/BartoszSolkaBD/OpenDrone/issues/341) does: "a ruined steel-frame hall: the roof sheets gone, rusty beams hanging bent against the sky, a heap of rubble with tall weeds growing out of it".

### Round 1: the place and its story

The story comes first, because it decides what lies where ([the lived-in places research](research/lived-in-places.md#5-the-design-briefs-history-section)). The Factory's story: an engineering works that closed, was stripped by thieves, used by scrappers and squatters, burnt in one corner, and is now where FPV pilots meet, with a fire pit and pallet seats.

| # | Question | Default to start from | The Factory's answer |
|---|---|---|---|
| 1 | What is the place, in one line? | your own concrete reading of the idea: "an abandoned light engineering works: a brick hall with a broken roof, and its yard" | an abandoned industrial site of about 200 × 100 m |
| 2 | Who built it, what for, and roughly when did it stop? | a working life with a clear purpose, closed about 15 years ago | a light engineering works with an overhead crane |
| 3 | How did it close, and who took what? | emptied of what was worth moving, then stripped by thieves | machines sold, cable and pipes stolen, a wall knocked through to drag the last machine out |
| 4 | Who came after, and what did they leave? | scrappers, then squatters for a while | scrappers' sorted metal, a squatters' camp |
| 5 | Did one big thing happen here? | a fire in one corner | the squatters' fire in the north-east corner |
| 6 | Who comes now, and where do they gather? | FPV pilots, with a fire pit by the main way in | pilots with a fire pit and a BBQ under a skylight inside the loading door |

### Round 2: flying, light and sky

| # | Question | Default to start from | The Factory's answer |
|---|---|---|---|
| 1 | How big is it, and how is it flown? | a 30 × 30 m slice first; the whole site 100 to 200 m across; freestyle and Free Flight (racing comes later) | about 200 × 100 m, freestyle and free roam |
| 2 | Which lines must stay open? | one loop through the whole site, and Gaps of every size the [World rules](context/world.md#rules) ask for: a few for the whoop only (0.5 to 1 m), most of 2 to 5 m, and a few dives of 15 m or more | the breach, the loading door, the man door, a vent hole, missing window panes, open skylights and a fallen roof bay to dive through |
| 3 | What time of day? | sunny mid-afternoon. Another time of day is another Map with its own bake ([ADR-0024](adr/0024-detailed-maps-are-made-from-scratch-and-lit-by-a-baked-atlas.md)) | a sunny day |
| 4 | Where is the sun, and how high? | about 35 to 40° up, from the side that sends it in through the roof's holes and the big openings, leaving one side shaded and damp | 37° up, from the south-west (`sun_azimuth` 215 °) |
| 5 | What kind of sky? | a deep, clear sky with a few clouds: `sky_strength` 0.25 and `sun_strength` 1.0, the Factory's | the Factory's `[light]` |
| 6 | How dark is it inside? | 5 to 6 stops under the outside, as the [camera deep dive](context/camera.md) asks | 5 to 6 stops |

### Round 3: the look, the limits and the cost

| # | Question | Default to start from | The Factory's answer |
|---|---|---|---|
| 1 | Three words for how it should feel? | your proposal, such as "abandoned, overgrown, still used" | believable at FPV speed; not photoreal |
| 2 | Two to four reference pictures? | your own three scenes in words for the person to approve, or the person's photos described in words | four, described in [#341](https://github.com/BartoszSolkaBD/OpenDrone/issues/341): a works with a striped chimney from the air, a ruined steel hall with a rubble heap, a concrete room with a thick layer of debris, a steel hall with light shafts and pallets |
| 3 | How wet and how old is it? | a temperate climate, a leaking roof, 10 to 20 years of decay | 10 to 20 years, moss and water on the shaded side |
| 4 | Anything to avoid? | no real brands, people or artists' work: graffiti and signs are generic and made by agents ([World rules](context/world.md#rules)) | the same |
| 5 | How much may it cost in speed? | about 8 ms a frame on the maintainer's M4 Mac mini at 2560 × 1440; over that, thin things with distance ([Speed](#speed)) | 8 ms, then accepted at 9.7 ms in Analog because the maintainer chose depth |
| 6 | What should a pilot remember from it? | two or three focal points in each 30 × 30 m, with open floor between ([what makes it feel real](research/lived-in-places.md#3-what-makes-it-feel-real)) | the fire corner, the pilots' hangout, the aisle and the fallen bay |

### What a round looks like

> **An abandoned swimming pool: round 1 of 3.** Reply "agreed", change any answer, or say "surprise me".
>
> 1. **What is the place?** I recommend: a 1970s indoor town pool, one hall with the pool drained, changing rooms beside it, a flat roof with roof lights.
> 2. **What was it, and when did it stop?** I recommend: a public pool, closed about 12 years ago when the roof started to leak.
> 3. **How did it close?** I recommend: emptied tidily (lockers and benches taken), then thieves took the copper and the boiler room's pipes.
> 4. **Who came after?** I recommend: skaters, who rode the empty pool, then squatters in the changing rooms.
> 5. **One big event?** I recommend: part of the flat roof fell into the deep end.
> 6. **Who comes now, and where do they meet?** I recommend: FPV pilots, on the spectators' gallery.

A pool full of water, or the same pool at night, would each be a gap the look check finds: the engine has no water surface and the bake has no lamps ([below](#3-the-look-check)).

## 2. The brief

The brief is one page, filed as a GitHub issue whose title starts "Spec: " (the [Delegator](agents/delegator.md) skips those when it looks for tickets). It holds what the person approved, nothing more: no tickets and no code. The look check's result and the tickets are added to it as comments.

```markdown
# Spec: <the Map's name>, a detailed Map made from scratch

**Place:** one line. **Id:** `opendrone/<id>`.

## Story (history in layers, oldest first)
1. Working life: ...
2. Closing and stripping: ...
3. Decay: how wet, how old, which side gets the weather.
4. Who came after: ...
5. The big event: ...
6. Who comes now, and where they gather: ...

## Size and flying
- The site: <width> × <depth> m; the first slice: <30 × 30 m, where>.
- Flown as: freestyle and Free Flight.
- Lines that stay open: the loop; each Gap with its size; the dives.
- The Launch Spot: where, facing what.

## Light and sky
- Time of day; sun elevation and azimuth; sky_strength and sun_strength.
- Inside against outside: <5 to 6> stops darker.

## References, in words (no pictures in the repo)
1. ...
2. ...

## Focal points
- A: <name>, <where>, <which layer of the story it shows>.
- B: ...

## Things to avoid
- No real brands, people or artists' work. ...

## Budget
- Frame time on the M4 Mac mini at 2560 × 1440: <8 ms>.
- Density: at least the Factory's (docs/designing-a-map.md, Density).
- Size: .glb, light atlas and pictures within <numbers>.

## Chosen by the agent ("surprise me")
- ...

## Out of scope
- Sound, racing, weather, moving or breakable things (as in docs/designing-a-map.md).
```

### The three gates

| Gate | The person approves | Until then |
|---|---|---|
| 1. The brief | the spec issue: story, size, flying lines, light, references, focal points, budget | no script and no ticket: the look check comes after this gate |
| 2. The slice | the slice, flown from a build and seen in 6 to 8 screenshots, and the layout plan for the rest | nothing outside the slice is built |
| 3. The finished Map | the whole Map, flown once | it doesn't merge |

A slice PR waits for the person with the `needs-maintainer` label, so it steps out of its Lane ([When a PR waits for the maintainer](agents/delegator.md#when-a-pr-waits-for-the-maintainer)). The Factory's slices 1 and 2 stayed open as references while the next was built.

## 3. The look check

Before any slice is built, list every visual feature the place needs and compare it with what the engine and the helpers in [`assets-src/lib/`](../assets-src/lib/) do today. Each feature that is missing, or that the place needs and no ticket has made yet, is one ticket of about an hour of authoring, filed before the slice. A slice ticket waits for them.

**Why.** The Factory's first slice ([PR #317](https://github.com/BartoszSolkaBD/OpenDrone/pull/317)) was built with what the engine had on 23 October. When the maintainer flew it the next morning, the light and materials were right, but the walls had no wear, the ground and roof were flat, there was no sky, the rotors didn't turn, the inside had no history, and the grass stopped along a razor line. Six tickets for the missing features and one for the next slice were filed at 07:37 UTC (the sky's, #315, was filed the night before), and the last feature merged at 13:33, before slice 2 could be finished: **about 6 hours of rework** ([#336](https://github.com/BartoszSolkaBD/OpenDrone/issues/336)). The second slice was then judged "like 10%" of the depth wanted, and a depth pass (real grass, the kit, relief) took two hours more. A look check of the spec's look against the engine would have found most of it before slice 1.

Go through this list for the place, and add what it needs beyond it:

| Feature | What the repo has today | A gap to file |
|---|---|---|
| Materials made for the place | generators in [`assets-src/generators/`](../assets-src/generators/), built from [`odtex.py`](../assets-src/lib/odtex.py) ([Materials made from scratch](building-a-map.md#materials-made-from-scratch)); 41 in the Factory | each material the place needs and no generator makes: pool tiles, asphalt, painted steel |
| No repeating blocks | `break_repeat`, `snap`, `turn_cells` and `macro_strength` in [`materials.toml`](../assets-src/materials.toml) ([Surfaces that don't repeat](building-a-map.md#surfaces-that-dont-repeat)) | none expected |
| Wear and stains | 18 decal materials and the rules of [`odwear.py`](../assets-src/lib/odwear.py) ([Wear](building-a-map.md#wear)) | a mark with a new cause, such as chlorine bloom or tyre rubber: a new decal generator |
| Wetness | `wetness` on a material, `decal_damp` and `decal_algae` ([Wet, rough and metal surfaces](building-a-map.md#wet-rough-and-metal-surfaces)) | standing water with a surface of its own (a flooded pool): not in the engine |
| Uneven ground and floors | [`odground.py`](../assets-src/lib/odground.py) and [`odrelief.py`](../assets-src/lib/odrelief.py) ([Relief](building-a-map.md#relief-floors-walls-and-roofs-with-depth)) | a shape neither makes, such as a pool's sloped floor with broken tiles |
| Walls and roofs with thickness | [`odopening.py`](../assets-src/lib/odopening.py), [`odbrick.py`](../assets-src/lib/odbrick.py), [`odroof.py`](../assets-src/lib/odroof.py), [`odframe.py`](../assets-src/lib/odframe.py) | a flat concrete roof, block walls, a curved shell |
| Real grass | the `scatter=` tag ([Grass](building-a-map.md#grass)): grass only | tall weeds, reeds or nettles: a second kind ([Adding a kind of plant](building-a-map.md#adding-a-kind-of-plant)) |
| Trees, weeds and ivy | [`odplants.py`](../assets-src/lib/odplants.py) | a tree or plant it doesn't make |
| Debris and heaps | the kit, [`odkit.py`](../assets-src/lib/odkit.py), and its rules, [`odscatter.py`](../assets-src/lib/odscatter.py) ([The kit and the scatter](building-a-map.md#the-kit-and-the-scatter)) | kit items the place needs: broken tiles, lockers, a trolley |
| A sky | made by the game from the Map's `[light]` ([What the game does with it](building-a-map.md#what-the-game-does-with-it)) | none for a day; a night sky with stars isn't there |
| Light with direction, and shafts | the baked sun through every opening ([Baked light](building-a-map.md#baked-light)); a shaft shows where it lands, on the floor and walls | a beam visible in the air needs haze or fog, which the engine doesn't draw ([ADR-0024](adr/0024-detailed-maps-are-made-from-scratch-and-lit-by-a-baked-atlas.md) keeps fog for later) |
| Light at night | none: the bake knows only a sun and a sky ([`odlight.py`](../assets-src/lib/odlight.py)) | lamps, a lit sign, a fire's glow |
| History you can read | [the lived-in places research](research/lived-in-places.md), kit items placed by hand (`place`), and the Factory's story objects, which [#346](https://github.com/BartoszSolkaBD/OpenDrone/issues/346) merged into the kit (one script, `odkit.py`: its hand-placed pieces and its scattered items share one builder each for a pallet, drum, tyre, can and bottle) | each story object the place needs that nothing makes yet |
| Moving things | none on a Map (spec #290 leaves moving and breakable things out); only the Quad's rotors turn | a flag, a swinging sign, running water |

**Sizing the tickets.** One behaviour a ticket, with its own acceptance criteria, about an hour of authoring. [#320](https://github.com/BartoszSolkaBD/OpenDrone/issues/320) (roughness, then occlusion and metal) and [#321](https://github.com/BartoszSolkaBD/OpenDrone/issues/321) (the blend, then decals) should each have been two: they took about 2 h 20 min and 1 h 55 min and ran one after the other ([#336](https://github.com/BartoszSolkaBD/OpenDrone/issues/336)).

## 4. The layout plan

Plan the site from above before the slice, so the slice is a real piece of the whole and not a set.

- **The frame.** Pick an origin and say it once: the Factory's is the hall's south-west corner on the floor, east +x, north +y, up +z. Every position in the brief, the plan and the script uses it.
- **The site.** The near site in metres (the Factory's yard is 66 × 78 m round a 30 × 30 m hall), the ground beyond it (to 70 m out, coarser), and the Backdrop to the horizon ([World rules](context/world.md#rules)).
- **The Launch Spot.** Open flat ground with 3 m clear and open sky, facing the main feature. The Factory keeps 3.8 m clear round it.
- **The flying lines.** The loop, each Gap with its size, each dive, and the way in to each focal point, drawn as rectangles and circles: they become the keep-clear zones of the kit's rules and of the relief ([What a flying line needs](#what-a-flying-line-needs-left-clear)).
- **The focal points.** Two or three in each 30 × 30 m, each showing one layer of the story, with open floor between them. List what lies at each, where, and in what state, as [the research](research/lived-in-places.md#6-the-factorys-story) does for the Factory.
- **The sun's side.** Mark which walls face away from the sun: they get the moss, the salt and the damp. Mark where rain falls through the roof: puddles and weeds go under it ([where things collect](research/lived-in-places.md#2-where-things-collect)).
- **The silhouette.** What stands against the sky from outside: a broken roof line, a chimney, a tank, trees. The Factory's slice added a 30 m chimney and hanging purlins for this.

Show the plan as a table of coordinates and as the plan picture `cargo xtask maps --previews` renders once a script exists ([Building and checking](building-a-map.md#building-and-checking)). It goes to the person with the slice, at Gate 2.

## 5. The slice

A slice is about 30 × 30 m of the site, built to the finished standard, with every feature the brief names: inside and outside, roof, walls, ground, at least one Gap of each size, a dive, and one focal point for each layer of the story. If the person likes it, the rest is more of the same.

**The order the Factory's script builds in** ([`assets-src/maps/factory/build.py`](../assets-src/maps/factory/build.py)):

1. The constants: the frame, the Launch Spot, the walls' runs and openings, the roof's holes, the focal points' positions.
2. `m.lightmap(texel_cm=10)` and `m.batch_decals()`, so the decals are a few parts.
3. The ground and the floor: `Ground`, then `Relief` with only the flying lines as its keep-clear zones, so the crumbled layer lies against the walls.
4. The walls: `broken_wall` for each, with pilasters and windows. `brick_face`, which came after the Factory's walls were built, goes only where the Quad passes close.
5. The roof: `Roof` sheets on trusses and purlins, with its holes. `BuckledRoof` on a broken `Frame` came later too: [#348](https://github.com/BartoszSolkaBD/OpenDrone/issues/348) is the ticket to give the Factory both, as its walls and frame still look flat in places.
6. The kit: one `Scatter` (the class in [`odscatter.py`](../assets-src/lib/odscatter.py), the kit's rules; the glossary's **Scatter** is the grown plants, as in [World and content](context/world.md)) for the hall with keep-clear zones on every flying line, every wall, every column and every story object, built on the relief's height; its rules; `apply` after the floor exists. A second for the yard.
7. The story objects, by focal point and by layer.
8. Plants and grass: the yard cut by how grassy it is, real grass on the grassy parts and none on paths.
9. Wear: `Wear` rules for every wall face and floor, then the story's own marks by `hand`.
10. A check that every material has a picture made from scratch, the Launch Spot, the export.

Then bake with `cargo xtask maps --bake <id> --samples 1024` (the Factory's took 487 s on the M4, and 635 s on an earlier, busier run) and write the Map's checks in `crates/pack/tests/<id>.rs`: the Launch Spot's ground and clearance, a line through every Gap at several heights, every drop clear from 25 m, the aisle clear, solid where it should be and drawn only where it should be. The Factory's are in [`crates/pack/tests/factory.rs`](../crates/pack/tests/factory.rs).

**Traps the Factory found** (from the hand-offs on [PR #317](https://github.com/BartoszSolkaBD/OpenDrone/pull/317) and [#341](https://github.com/BartoszSolkaBD/OpenDrone/issues/341)):

- A face smaller than a texel bakes black: the kit's `apply` stretches each item's charts to two texels (`light_least`).
- Put keep-clear zones on walls and columns too, or a heap grows through them; give the relief only the flying lines.
- The kit's own materials are downloaded ones; a Map made from scratch maps them to its own (the Factory's `KIT_MATERIALS`) and needs no new picture.
- Every decal is a part to draw: 253 decals were 253 draws until `batch_decals` made them 19.
- Grass only grows on triangles that face up; leave paths, hard standing and the Launch Spot untagged, and stop real grass where the camera never comes near (the Factory's `GRASS_REACH`).
- Take the final bake (1024 samples) before the screenshots: a 16-sample bake is for quick looks only.
- `--render-views`: one or two views a run ([#308](https://github.com/BartoszSolkaBD/OpenDrone/issues/308)), and `look=digital` rather than a plain view on a baked Map ([#316](https://github.com/BartoszSolkaBD/OpenDrone/issues/316)).

## 6. Handing a slice over

A slice PR is for the person to fly. It has:

- **One build.** The `build` and `needs-maintainer` labels; the Build workflow makes the downloads for macOS, Windows and Linux ([Builds and releases](builds-and-releases.md)). The PR says how to run it from the branch: `./play --map <id>` ([Starting the game from any checkout](pilot-guide.md#starting-the-game-from-any-checkout)).
- **6 to 8 screenshots** from `--render-views` in a release build, with the final bake, on a branch `screenshots/<N>` that is never merged, linked from the PR as PRs [#317](https://github.com/BartoszSolkaBD/OpenDrone/pull/317) and [#335](https://github.com/BartoszSolkaBD/OpenDrone/pull/335) did. The Factory's set: the Launch Spot in Analog; the site from outside; top-down from 40 m (repeats show at once); inside, low along the floor; each focal point; a whoop at a small Gap in Analog; a 5″ dive in Analog. Some from FPV distance (1 to 5 m), not only wide views.
- **The numbers:** frame time against the last slice and Bando ([Speed](#speed)), and the size and density tables below filled in for the slice.
- **What you are unsure of,** and the questions for the person.

## 7. Turning feedback into tickets

The person's feedback goes on the spec as a comment, in their words. Then:

- **Sort it.** A missing feature is a look-check ticket for the engine or a helper. A change to the Map is part of the next slice's ticket. A choice is a question for the person.
- **Size each ticket to about an hour,** one behaviour, as for the look check.
- **Say what each finding becomes.** After slice 1: "no sky" became [#315](https://github.com/BartoszSolkaBD/OpenDrone/issues/315), "too clean, no history" became the research [#318](https://github.com/BartoszSolkaBD/OpenDrone/issues/318), "everything flat" and "razor-cut grass edge" became the geometry helpers [#322](https://github.com/BartoszSolkaBD/OpenDrone/issues/322). After slice 2, "like 10%" became real grass, the kit and relief ([#338](https://github.com/BartoszSolkaBD/OpenDrone/issues/338) to [#340](https://github.com/BartoszSolkaBD/OpenDrone/issues/340)) and slice 3 ([#341](https://github.com/BartoszSolkaBD/OpenDrone/issues/341)), with the reference pictures described in words.
- **Keep what the person liked.** Slice 3 kept slice 2's entrance with its broken bricks because the maintainer named it.

## 8. Growing it into the full Map

The Factory hasn't been grown past its slice yet, so this part is a plan, not a record:

- **An area at a time,** each about the slice's size and its own ticket, each with one layer of the story strongest in it ([the whole site](research/lived-in-places.md#the-whole-site)).
- **The light atlas.** One atlas holds at most 8192 × 8192 texels (67 million). The Factory's slice and yard used 12.0 million at 10 cm a texel. Use coarser light (`light_cm=`) on outsides, roofs and far ground before asking for a second atlas, which doesn't exist yet ([Baked light](building-a-map.md#baked-light)).
- **Real grass stops where the camera never comes near,** and nothing beyond 60 m is drawn anyway ([Grass](building-a-map.md#grass)).
- **Benchmark each area as it lands,** from the worst place in it, not only from the Launch Spot.

## 9. The benchmark

Measure the frame on the maintainer's M4 Mac mini, in a release build, at 2560 × 1440:

```sh
cargo run --release -p opendrone -- --map <id> --camera-cost --seconds 10 --size 2560x1440
```

It runs each Video Look for 10 seconds from the Launch Spot and counts the frames: the frame time is the seconds over the frames (10 s over 1,034 frames is 9.7 ms). Run the new Map, the last slice and Bando back to back, two or three times each, and say the load on the computer (`uptime`): other agents' builds make the numbers rough. Timings are never a CI gate ([ADR-0013](adr/0013-performance-gates-count-work-not-time.md)); they go in the PR.

## The benchmarks

All numbers below are from the Factory as it ships in [PR #350](https://github.com/BartoszSolkaBD/OpenDrone/pull/350) ([#346](https://github.com/BartoszSolkaBD/OpenDrone/issues/346)): the third slice, the hall corner and its yard, with one kit script and a new bake, from branch `ticket/341-factory-slice-v3`. The speed table also keeps the measurement of the slice the maintainer tested (commit `d599e26`, before the kit script was merged and the light re-baked), beside the merged one. [How they were measured](#how-these-numbers-were-measured) is at the end.

### Speed

Measured by [#341](https://github.com/BartoszSolkaBD/OpenDrone/issues/341)'s author with the command above, the same build back to back, a load average of about 5:

| | Analog | Digital |
|---|---|---|
| Factory as merged ([PR #350](https://github.com/BartoszSolkaBD/OpenDrone/pull/350)) | 9.5 ms (1,051 and 1,049 frames in 10 s) | 8.1 and 8.3 ms (1,232 and 1,204) |
| Factory slice 3 at `d599e26` (the one the maintainer tested) | 9.7 to 9.8 ms (1,034 and 1,021 frames in 10 s) | 9.1 ms (1,101) |
| Factory slice 2 ([PR #335](https://github.com/BartoszSolkaBD/OpenDrone/pull/335)) | 7.9 ms (1,261 and 1,257) | 6.8 ms (1,468 and 1,456) |
| Bando, same run as the merged Factory | 5.3 ms (1,898 and 1,895) | 4.5 ms (2,236 and 2,228) |
| Bando, same run as slice 3 at `d599e26` | 4.8 ms (2,073) | 4.1 ms (2,459) |

- **The budget** is about 8 ms a frame (at least 125 frames a second), agreed for the depth pass. The Factory's slice 3 was 1.7 to 1.8 ms over it in Analog (the merged one measured 1.5 ms over, on a quieter computer), and the maintainer accepted that because he chose depth: "I think it's fine, no need to address" ([#346](https://github.com/BartoszSolkaBD/OpenDrone/issues/346)). A new Map aims at 8 ms.
- **The frame is bound by the processor, not the graphics card:** it took as long at 640 × 360 as at 2560 × 1440. What costs is the number of things drawn, not triangles. Slice 3 draws 243 parts from its `.glb` and 136 cells of grass, about 380 in all.
- **Where the time went:** before any cut, slice 3 took 11.4 ms. Leaving out every decal saved about 2.8 ms, and leaving out all the grass about 3.1 ms (522 cells then), both measured before the cuts below.
- **What to cut first,** in this order:
  1. **Grass reach:** stop real grass sooner round the site (`GRASS_REACH` in the Factory's script); 522 cells became 136.
  2. **Grass cell size:** bigger cells are fewer draws, but the 8 m cell is a constant in [`grass.rs`](../crates/opendrone/src/grass.rs): a game ticket.
  3. **Kit parts:** a bigger `cell` in the kit's rules merges more items into each part.
  4. **Plaster chips:** they are a third of the kit's triangles and show least from a few metres; lower `per_m2` or their weight in the mix.
  5. **Decals:** always `batch_decals()`; thin the rule-made marks that read least at speed (the Factory keeps one in three runs and cracks, one in two scratches and droppings).
- **What other features cost**, measured on test Maps ([Building a Map](building-a-map.md)): real grass about +1 ms, relief about +0.5 ms, a whole hall floor of kit about what Bando costs in all, decals and blends +0.05 ms for 13, rough and metal surfaces +0.1 ms.

### Size: the Factory hall corner and yard

The near site is the yard's rectangle, 66 × 78 m (5,148 m²), with the 30 × 30 m hall in it; the file also holds ground to 70 m out, the Backdrop, a chimney and trees. "Per 100 m²" is the whole file over the near site, a first guess for a new Map of another size: measure it again once built.

| What | Factory slice 3 | Per 100 m² of site | Grows with | Where it comes from |
|---|---|---|---|---|
| `factory.glb` | 26.3 MB (slice 2: 13.8 MB; Bando: 0.9 MB), SHA-256 `f48f82d3…` | 0.51 MB | triangles: about 52 bytes each | the file |
| Triangles | 508,192: 469,004 drawn, 15,748 for collision only, 23,440 in decals | 9,900 | detail: the kit is 195,000 (with its heaps' cores), plants 86,000, ground 77,000, roof 50,000, walls 40,000 | the build's own count and the `.glb` |
| Parts to draw | 243 (224 shapes and 19 decal parts), and 98 for collision only | | materials × kit cells, not area | the `.glb` |
| Light atlas | 2048 × 5848 texels at 10 cm, 158,140 charts filling 93% of it; 33.0 MB as EXR; 95.8 MB in the graphics card (half floats, 8 bytes a texel) | 233,000 texels; 0.64 MB of EXR | surface area over the texel size squared, and the kit's small charts | `factory.bake.toml` and the build's unwrap |
| Bake | 487 s at 1024 samples on the M4 (635 s on the slice before the merge, on a busier computer) | | texels × samples | `factory.bake.toml` |
| Grass | 32,821 clumps in 136 cells of 8 m (32,930 expected from the tags, on 7,175 m² of tagged ground); about 47 MB in the graphics card at 45 corners of 32 bytes a clump | 640 clumps | grassy area × density, cut at `GRASS_REACH` | the game's count, in the [#341](https://github.com/BartoszSolkaBD/OpenDrone/issues/341) hand-off; the tags in the `.glb` |
| Decals | 253, drawn as 19 parts, 23,440 triangles | | wall and floor area × wear level | the build |
| Kit | 6,961 items (4,887 in the hall, 2,074 in the yard), 180,890 triangles, 80 merged parts | | floor area × `per_m2` | the kit's `stats()` |
| Pictures | 41 materials and the large picture: 42 textures, 11.7 MB on disk (9.7 MB surfaces, 2.0 MB decals); 1 at 2048 pixels (the floor), 9 at 1024, 32 at 512 | | one set per material, not per area | `assets-src/generated/` |

### Density

So that a new Map can say "match or exceed the Factory's density" with a number. The hall's floor is 873 m² (29.55 m square inside the walls), the yard 4,221 m² (the near site less the hall), and the walls' faces 2,103 m² (both sides of the four walls and their gables, measured on the `.glb`).

| Per 100 m² | Hall floor | Yard | Walls (each face) |
|---|---|---|---|
| Kit items | 560 | 49 | |
| Decals | 6.5 (57 in all) | 0.19 (8) | 8.7 (184) |
| Grass clumps | none inside | 625 | |
| Story objects | 156 pieces placed by hand over floor and yard, about 3 per 100 m²: the plinths and the crane rail's brackets, the camp, the hangout, the crashed-quad bits, the gate and the yard's things | | |

- **Kit items by kind:** 2,895 plaster chips (a third of the kit's triangles), 1,353 brick chunks, 1,281 concrete chunks, 693 bricks, 281 concrete lumps, then a few hundred cans, bottles, plastic scraps, planks, boards and roof-sheet pieces, and a handful of pallet stacks, drums, tyres and bolt plates.
- **Grass:** 7 clumps a square metre, 14 to 50 cm tall, where the yard is grassy (3,673 m²), 2.5 where it is thin over gravel, 9 and up to 90 cm on the rubble heaps, and none on the worn path, the hard standing or round the Launch Spot. About 9 in every 10 m² of the yard grows grass. Beyond the yard, 2 a square metre to `GRASS_REACH`.
- **Decals by where they lie:** 184 on walls (water runs and stains, moss, salt, scratches, rust, soot, tags and stencils, cracks, lichen, cobwebs), 57 on the hall's floor (damp patches and puddles, algae, scuffs and drag marks, droppings, oil, paint flakes, a scorch), 8 in the yard (puddles, algae, oil) and 4 under the roof (soot).

### What a flying line needs left clear

- **The Factory's hall keeps 9.6% of its floor clear for flying,** and the kit's rules keep 25.4% clear in all (the flying lines, the plinths and the story objects' places).
- **Its zones:** the Launch Spot, a circle of 3.8 m; the aisle, a strip 3 m wide down the hall; each doorway's and the breach's way in, a little wider than the opening and reaching 1 to 3 m each side of the wall; a circle of 0.6 to 0.7 m round each drop through the roof.
- **Inside a zone,** the kit puts nothing, the relief rises no more than 2 cm and eases back over 60 cm, and no bent purlin hangs into it ([Relief](building-a-map.md#relief-floors-walls-and-roofs-with-depth)).
- **The checks prove it:** a line through every Gap at several heights (0.35 and 1.5 m in the aisle), and every open skylight a clear drop from 25 m to under 0.3 m above the floor.

### The immersion checklist

The maintainer's test, in his words. Before Gate 2 and Gate 3, check each item in the screenshots and name the tool that makes it:

| The test | The tool that makes it | How to check |
|---|---|---|
| Nothing in view is a flat surface | `Ground`, `Relief` (`crumble`, `rut`, `pothole`, `under_heap`), `graded_mesh`, `BuckledRoof`, `broken_wall`'s bulge and lean ([Relief](building-a-map.md#relief-floors-walls-and-roofs-with-depth), [Geometry that isn't boxes](building-a-map.md#geometry-that-isnt-boxes)) | a view low along the floor toward the light |
| No large area looks like the same picture | `break_repeat`, `snap`, `macro_strength` and the large picture in `materials.toml`; `blend=` with a `WanderingEdge` ([Surfaces that don't repeat](building-a-map.md#surfaces-that-dont-repeat), [Blending two materials](building-a-map.md#blending-two-materials)) | top-down from 40 m |
| The grass is real | `scatter=("grass", density, (shortest, tallest))` on the ground ([Grass](building-a-map.md#grass)) | a view from 1 m up in the yard |
| Walls have thickness, with holes and broken edges | `broken_wall` with `Opening`s and `loose_bricks`; `brick_face` near flying lines ([Broken openings](building-a-map.md#broken-openings), [Brick walls with depth](building-a-map.md#brick-walls-with-depth)) | a whoop's view at a breach |
| The floor has mess in clusters with clear gaps | the kit and its rules: `heap`, `wall_foot`, `strew`, `pallets`, `boards`, `anchor_row`, with keep-clear zones ([The rules](building-a-map.md#the-rules)) | inside, low along the floor |
| The light has direction and shafts | the baked sun through the roof's holes (`Skylight`, `Tear`, `missing` in [`odroof.py`](../assets-src/lib/odroof.py)) and the big openings ([Baked light](building-a-map.md#baked-light)) | a view inside toward the sun |
| The sky exists | the Map's `[light]` in `map.toml` ([What the game does with it](building-a-map.md#what-the-game-does-with-it)) | from the yard and through the roof |
| The place has a history a visitor can read from its objects | [the lived-in places research](research/lived-in-places.md), story objects at each focal point, `Wear` rules and `hand` marks ([Wear](building-a-map.md#wear)) | one view of each focal point: can you tell what happened there? |
| The silhouette from outside is not a box | `Frame`'s broken purlins and missing sheets ([Buckled roofs](building-a-map.md#buckled-roofs-on-a-broken-frame)), tall shapes such as the Factory's chimney, trees | from the yard, 30 m out |
| Scale cues are present | kit items at their real size (a Euro pallet 1.2 × 0.8 m, a 200 litre drum, a 215 mm brick), doors and windows at real heights; the previews' 1.8 m figures | a Video Look view landed on the Launch Spot |

## Running it as tickets

The Factory ran as tickets through the [Delegator](agents/delegator.md), each its own PR with a Reviewer. What the effort learned ([#336](https://github.com/BartoszSolkaBD/OpenDrone/issues/336) puts the first three into the Delegator's pages):

- **About an hour a ticket,** one behaviour each, with its own acceptance criteria. A ticket that changes the engine or a shared helper does one thing.
- **Build ahead on an open PR when files don't overlap.** A ticket whose blocker is an open PR may build on the blocker's branch when it changes none of the files that PR changes (`gh pr diff <P> --name-only`) and really needs its code; it opens its own PR only after the blocker merges. [#321](https://github.com/BartoszSolkaBD/OpenDrone/issues/321), [#323](https://github.com/BartoszSolkaBD/OpenDrone/issues/323), [#340](https://github.com/BartoszSolkaBD/OpenDrone/issues/340) and [#341](https://github.com/BartoszSolkaBD/OpenDrone/issues/341) did, with the maintainer's approval; until #336 lands, the Delegator's page still says to wait.
- **Hot files are edited by adding sections only:** `docs/building-a-map.md`, `crates/xtask/src/main.rs`, `CONTEXT.md`, `docs/SUMMARY.md`, the deep dives, `assets-src/materials.toml` and the list of Maps in `crates/opendrone/src/main.rs`. New material goes in a new section, file or module.
- **Lanes:** a Map's script, its pictures and its checks are the Content Lane; the engine is the Game Lane; this page is Docs ([Lanes](agents/delegator.md#lanes)). A slice PR that waits for the person steps out of its Lane.
- **Models:** the slices, the research, the decals ([#321](https://github.com/BartoszSolkaBD/OpenDrone/issues/321)) and the depth pass ran on the Strongest tier, the other engine tickets on Standard ([Model tiers](agents/delegator.md#model-tiers)).

### What took the time

From each PR's `createdAt` and `mergedAt` and each ticket's "Started" comment on GitHub, in UTC, on 23 and 24 October 2026:

| Stage | Tickets | From | To | Took |
|---|---|---|---|---|
| The spec and the ADR | [#290](https://github.com/BartoszSolkaBD/OpenDrone/issues/290), PR [#296](https://github.com/BartoszSolkaBD/OpenDrone/pull/296) | 23 Oct, PR opened 14:58 | merged 15:04 | a grilling, then 6 min |
| The enablers: a screenshot command, generated textures, the bake, baked light in the game, no repeats | [#291](https://github.com/BartoszSolkaBD/OpenDrone/issues/291) to [#294](https://github.com/BartoszSolkaBD/OpenDrone/issues/294), [#304](https://github.com/BartoszSolkaBD/OpenDrone/issues/304) | filed 14:58 | last merged 21:24 | 6 h 26 min |
| Slice 1 | [#295](https://github.com/BartoszSolkaBD/OpenDrone/issues/295), PR [#317](https://github.com/BartoszSolkaBD/OpenDrone/pull/317) | started 21:25 | PR 22:40, passed review 23:04 | 1 h 15 min to the PR |
| Waiting for the maintainer | | 23 Oct 23:04 | 24 Oct 07:37 | overnight |
| The rework the look check would have saved | [#315](https://github.com/BartoszSolkaBD/OpenDrone/issues/315), [#318](https://github.com/BartoszSolkaBD/OpenDrone/issues/318) to [#323](https://github.com/BartoszSolkaBD/OpenDrone/issues/323) | filed 07:37 (#315 the night before) | last merged 13:33 | 5 h 56 min |
| Slice 2 | [#324](https://github.com/BartoszSolkaBD/OpenDrone/issues/324), PR [#335](https://github.com/BartoszSolkaBD/OpenDrone/pull/335) | started 13:12 | PR 14:46, passed review 15:48 | 1 h 34 min to the PR |
| The depth pass: real grass, the kit, relief | [#338](https://github.com/BartoszSolkaBD/OpenDrone/issues/338) to [#340](https://github.com/BartoszSolkaBD/OpenDrone/issues/340) | filed 15:40 | last merged 17:41 | 2 h 01 min |
| Slice 3 | [#341](https://github.com/BartoszSolkaBD/OpenDrone/issues/341) | started 16:43, building ahead | stopped 17:46 at the usage limit, no PR | |

- **Authoring was quick:** from "Started" to the PR took under an hour for most tickets and 1 h 35 min at most.
- **Chains were slow.** #320 → #321 → #323 → #324 ran one after the other: the rework's six hours. Smaller tickets and building ahead shorten a chain.
- **Hot files held PRs in review:** [PR #301](https://github.com/BartoszSolkaBD/OpenDrone/pull/301) (the bake) for 3 h 17 min and [PR #302](https://github.com/BartoszSolkaBD/OpenDrone/pull/302) for 2 h 36 min, both editing `docs/building-a-map.md` and `crates/xtask/src/main.rs`.
- **Skipping the look check cost the most:** about 6 hours, then 2 more for depth the reference pictures would have asked for.
- **Speed came last and cost a round of cuts:** slice 3 started at 11.4 ms. Measure at each slice, not at the end.

## What this guide leaves out, and why

- **Sound.** A Map's own Background Sound is a later piece of work; [spec #290](https://github.com/BartoszSolkaBD/OpenDrone/issues/290) left it out.
- **Racing.** Courses of Gates and Scenarios on a detailed Map come after the Map exists.
- **Weather and other times of day.** Each is a separate Map with its own bake ([ADR-0024](adr/0024-detailed-maps-are-made-from-scratch-and-lit-by-a-baked-atlas.md)), so it is a new Map through this same guide, not a setting.
- **Moving and breakable things.** Nothing on a Map moves or breaks yet; the engine has no way to do it with baked light.
- **Redoing the Skate Park and Bando.** The maintainer chose to leave them as they are; only new detailed Maps follow this page.
- **Speed tuning beyond the benchmark** (detail that changes with distance, culling): it waits until a full Map shows it is needed ([spec #290](https://github.com/BartoszSolkaBD/OpenDrone/issues/290)).

## How these numbers were measured

- **The build.** The Factory's script on the merged branch, run with the pinned Blender 5.2.2, gives the very same `.glb` as the committed one (SHA-256 `f48f82d3…`; `d599e26`'s was `38bc0230…`). A wrapper around it recorded every decal (its material, centre and facing), every kit item (its kind, place and triangles) and each story object added by hand. The script prints most of this itself: `MapBuilder.stats()`, the decals by kind and the kit's `stats()` for the hall and the yard, so a new Map's author gets the same numbers by running its script with Blender as its first lines say.
- **The file.** A plain Python reader of the `.glb` counted the parts, the triangles, the decal parts, and the grass the game will grow: the up-facing area of each tagged part times its density, as [`grass.rs`](../crates/opendrone/src/grass.rs) counts it. The game's own count (32,821) is from the #341 hand-off.
- **The areas.** The hall's floor is the inside of its walls; the yard is the script's yard rectangle less the hall; the walls' area is the vertical faces of the four walls and the gables in the `.glb`. A decal counts for a floor or the yard when it faces up, for a wall when it faces sideways, and for the roof when it faces down.
- **The light.** `factory.bake.toml` and the unwrap's own report.
- **The frame times** of the slice at `d599e26` are the #341 author's. The merged Factory's are the #346 author's: the same release build of the game, `--camera-cost --seconds 10 --size 2560x1440`, the Factory and Bando run back to back twice, with a load average of about 1.8 (the #341 runs were at about 5), so Bando is not the same number as before and the two sets are not to be compared to the tenth of a millisecond. The grass count (32,821) and the numbers of the other rows that did not move are the #341 hand-off's.

The scripts are throwaway and not in the repo. Measure a new Map the same way, and add its row to these tables when it lands.
