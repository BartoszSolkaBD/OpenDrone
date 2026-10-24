# What makes an abandoned site read as a place with a history

Research for [#318](https://github.com/BartoszSolkaBD/OpenDrone/issues/318), under the Factory's spec, [#290](https://github.com/BartoszSolkaBD/OpenDrone/issues/290). The rules it works within are in [ADR-0024](../adr/0024-detailed-maps-are-made-from-scratch-and-lit-by-a-baked-atlas.md) and the [World and content](../context/world.md) rules. It is the first section of the guide that lets an agent turn a person's idea into a detailed Map. Use it as a checklist.

**Why it exists.** The maintainer flew the Factory's first slice ([PR #317](https://github.com/BartoszSolkaBD/OpenDrone/pull/317)) on 24 October 2026. The walls, the light and the materials were right. The inside was "too clean: it lacks history that sells that stuff was happening here, and later it became a bando, and people came to fly, drink, have a BBQ". A pilot reads a place's past in about a second, at speed. A place with no past reads as a set.

**Where it comes from:** general knowledge of urban-exploration photography, industrial decay, metal theft and squatting, FPV freestyle in abandoned buildings, and how games tell a story with a room's contents (environmental storytelling). [Sources](#sources) are linked at the end. No outside text or pictures are copied into the repo.

**Words.** A *prop* is a loose object the Map's script builds from shapes: a pallet, a bottle, a mattress. *Wear* is a mark on a surface: a stain, moss, soot, a scuff. Wear comes from decals and blends ([#321](https://github.com/BartoszSolkaBD/OpenDrone/issues/321)), placed by seeded rules with hand-placed marks on top ([#323](https://github.com/BartoszSolkaBD/OpenDrone/issues/323)). "Bando" here is FPV slang for an abandoned building. It isn't our Map called Bando.

## 1. The layers of history

A place is read like the ground under an archaeologist. Each layer sits **on top of** the ones before it and partly covers them. Never put an older trace on top of a newer one. Soot runs over the old paint, not under it. Moss grows over the old tag, not the new one.

For each layer, the list says what it leaves behind. Tick off at least one strong trace per layer the brief names.

### Layer 1: the working life (oldest)

What the place was for leaves marks that outlive everything else, because they are heavy, built in or worn into the surfaces.

- **Fixings with nothing on them:** machine plinths (raised concrete blocks) with cut-off anchor bolts; brackets, hooks and cable trays on walls and trusses; crane rails on the pillars; pipe brackets with no pipe; a fuse board's empty steel box.
- **Shadows of what stood there:** a clean or pale rectangle on the floor or wall where a machine, a cabinet or a sign hung; a dark oil ring under where a machine ran.
- **Paint and signs:** a painted dado band (a darker band along the bottom metre or so of an inside wall); faded walkway lines on the floor; bay numbers stencilled on pillars; safety signs; a works name on the gable, faded. All of it generic, as the [World and content](../context/world.md) rules say.
- **Wear from use, where people and machines went:** thresholds worn hollow; door frames and pillars scuffed at forklift height (0.3–1.2 m); tyre marks on the floor along the old aisle; steel kick plates dented; floor paint worn through on the main route and still bright under the plinths.

### Layer 2: closure and stripping

When a site closes, the owner takes the valuable and movable things. Then metal thieves come for what's left. They take what sells and can be carried or cut quickly.

- **Taken first:** copper (cable pulled from trays and conduits, cut ends hanging; pipes; the earth bars), brass and lead (taps, valves, flashing off the roof), machinery and motors, light fittings, radiators, aluminium window frames, anything with a plug.
- **Left behind:** heavy, cheap or built-in things. Concrete plinths, crane rails and steel beams too big to cut by hand, brick, broken glass, rotten timber, an old office chair, a filing cabinet on its side, a tyre, an oil drum, paper and cardboard, broken fluorescent tubes.
- **Signs of the taking:** empty cable trays and torn-open junction boxes; pale lines on painted walls where pipes ran; bolts cut flush with an angle grinder (a bright cut face, rusted later); drag marks on the floor from a doorway to where a machine stood; a wall knocked through to get something big out.
- **Signs of the closing:** boarded windows (plywood or steel sheet), later prised off; a padlocked door; a "keep out" or "danger" board, generic.

### Layer 3: decay

Water does most of the damage. It comes in where the roof and windows are broken, runs down and pools at the lowest point.

- **Water:** dark stains and streaks running down from each window sill, roof edge, broken gutter and down-pipe; puddles or damp dark patches on the floor directly under roof openings; white salt bloom (efflorescence) low on brick and concrete walls; plaster bubbling and falling off where the wall stays wet.
- **Rust:** orange runs below every steel fixing, bolt and seam; more rust on steel under the roof openings than under the whole roof; rust stains on the floor under a fallen sheet.
- **Moss, algae and lichen:** in damp places the sun doesn't reach. That is the north-facing side in the northern hemisphere, but better said: the side away from the Map's sun, low down, in corners and where soil collects.
- **Plants:** weeds and grass in floor cracks where light and rain fall; tough young trees (buddleia and birch are the classic ones on old industrial sites) on wall tops, in gutters and in rubble; ivy up outside walls.
- **Collapse:** fallen roof sheets and purlins under the openings they came from; spalled concrete (flakes off the surface, sometimes showing a rusty rebar); fallen plaster in a strip along the foot of its wall.
- **Birds and animals:** white droppings in lines on the floor under the trusses and beams they perch on; feathers; a nest on a truss or a ledge; leaves and litter blown into drifts against walls.

### Layer 4: scrappers and squatters

People with nowhere better to be use the shelter. They leave the place more broken, and they leave fire.

- **Forced ways in:** a steel sheet over a door bent back at one corner; a padlock hasp cut; boards prised off a low window; a gap broken in a fence.
- **Scrappers' traces:** small piles of sorted metal they didn't take (steel offcuts, a heap of aluminium), and **burnt cable**: scrappers burn the plastic off copper cable, which leaves a black ring, melted sheath and blackened wire.
- **Squatters' traces:** a mattress or a bed frame, blankets, cardboard laid on the floor for warmth, a windbreak of pallets or boards, a crate table with candle stubs, plastic bottles, food tins, a shopping trolley, clothes.
- **Fire:** a fire from a squatters' camp or a cable burn that got out of hand. Soot runs up the wall above it and fans out under the roof; the roof sheets and trusses above are blackened; plaster is cracked by heat; wood is charred, and steel things in the fire survive (a bed frame's springs, a trolley, tins).
- **Graffiti from this time:** older tags, faded and partly covered by damp, moss and the soot.

### Layer 5: visitors and flyers

The last people to come leave the freshest traces, and they make the place feel used today. For an FPV Map, these are pilots: a bando is a place to fly freestyle and meet up.

- **A hangout:** a fire pit (a ring of bricks or a cut-down oil drum, with ash and half-burnt wood); a BBQ (a grill grate on bricks, a disposable grill, or half an oil drum on legs); seats and tables made of what was lying around: stacked pallets, plastic bottle crates, a big wooden cable drum as a table.
- **Litter by the seats:** bottles and cans (some crushed), a crate of empties, a bin bag, a lighter, food packaging.
- **Their way in:** a repaired gate in a fence or wall (not a racing gate: chained or strapped back on its hinges, with a newer padlock or chain on rusty steel), or a cut fence; a desire path worn through the grass from it to the hangout.
- **Pilot things:** a launch pad (a square of plywood or a mat, often with a painted mark); a battery-charging spot (a table with tape, a power bank, zip ties); crashed-quad bits: snapped props, a bent antenna, a broken frame arm, a lost whoop stuck high on a truss or in a window grid, zip ties and tape off a repair.
- **New tags,** brighter and lower down, near where people sit and enter, painted over older ones.

### Layer 6: nature, the last layer

Nature keeps going over everything, including the newest layer, a little. Grass grows through the pallets left last summer. Leaves lie in the cold fire pit. A sapling stands in the rubble. Moss covers the oldest layers most and the newest least.

## 2. Where things collect

Things don't land evenly. They gather where forces put them. Use these as placement rules: each names where, and why.

| What | Where | Why |
|---|---|---|
| Debris, rubble, leaves, litter | In corners and in a strip along the foot of walls; against anything standing on the floor; in the lee of a doorway | Wind and feet push loose things to the edges; corners trap them |
| Fallen pieces | Directly under where they came from: roof sheets under the roof opening, bricks either side of a broken wall, plaster below its wall | Gravity |
| Water stains and streaks | Below window sills, roof edges, gutters, down-pipes, cracks in the roof; spreading downwards and fanning wider | Water runs down and spreads |
| Puddles, damp, moss on the floor | Under roof openings; in low spots; at the foot of walls on the side away from the sun | Water pools low and dries slowest in shade |
| Moss and algae on walls | Low down, on the shaded side, in corners, where soil and leaves have built up | Damp and shade |
| Plants inside | In floor cracks under roof openings and near broken walls and doors | They need light and rain |
| Rust runs | Below each steel fixing, bolt, seam and beam end | Water carries rust down |
| Soot | Above fires, fanning upwards and outwards, blackest just above the flame and on the ceiling above | Smoke rises |
| Bird droppings | In lines on the floor under trusses, beams and ledges | Birds perch there |
| Paths, scuffs and wear | Worn straight across floors between doors, Gaps and the places people use; floor paint gone on the path and kept beside it | People take the shortest route |
| Litter | Around seats and fires, and dropped along paths | People drop things where they stop |
| Graffiti | At reachable heights (0.5–2.2 m) near ways in, on smooth walls; a few high ones where someone climbed | People paint where they can stand |

## 3. What makes it feel real

- **A few strong focal points,** not an even spread. A pilot remembers "the fire corner" and "the camp by the door", and the empty floor between them makes them stand out. Aim for two or three clusters in a 30 × 30 m space, with open floor between.
- **Things placed for a reason.** Every prop should answer "why is it here?": the pallets are seats because they face the fire; the bricks are by the wall because the wall broke; the mattress is in the driest corner.
- **Asymmetry.** One end is worse than the other. One side gets the weather, the other doesn't.
- **Scale variety:** big shapes (a fallen roof, a plinth), middle ones (pallets, drums, a mattress), small ones (bottles, bricks, props off a quad). A pilot at speed reads the big ones; up close and landed, the small ones sell it.
- **Old next to new.** A shiny can on a mossy plinth. A new padlock on a rusty gate. A bright tag over a faded one. Recent use beside old decay says "people still come here".
- **Every surface has its own age.** Floor paint under a plinth is newer than paint in the aisle. A newer brick patch in an old wall. Steel under the roof is less rusty than steel under an opening.
- **Wear that follows a cause.** Every stain, scuff and patch of moss has a source you could point to.

## 4. What breaks it

- **Perfect right angles and flat planes:** razor-straight edges where grass meets dirt, broken openings with square corners, floors and roofs that are perfectly flat.
- **Evenly sprinkled props:** the same density everywhere, or props spaced on a grid. Random scatter with no cause reads as fake as a grid does.
- **Every surface the same age:** one uniform layer of dirt over everything.
- **Anything too clean:** a floor with no litter or dust in the corners, walls with no stains below the windows.
- **Wrong layer order:** moss on a fresh tag, soot under old paint, a clean new pallet under a fallen roof sheet.
- **Things that couldn't have got there:** a heavy machine with no way out left in the middle, a fire under an intact roof with no soot.
- **Copies:** the same bottle, crate or pallet repeated at the same angle. Vary turn, tilt, size and wear for each one.

## 5. The design brief's "History" section

Every design brief in the guide has a "History" section. The agent asks the person these questions, at most six. Each one has a default, so "surprise me" works; the defaults below suit an abandoned industrial site like the Factory. The agent never asks what it can look up, such as where the sun is (the Map's `[light]`) and so which side is shaded.

1. **What was this place for, and roughly when did it stop?** Default: a light engineering works, closed about 15 years ago.
2. **How did it close? Was it emptied tidily, or abandoned in a hurry?** Default: emptied of machines, then stripped by thieves.
3. **How wet and how old is it now?** Default: a temperate climate, a leaking roof, 10–20 years of decay.
4. **Who came after, and what did they do?** Default: scrappers, then squatters for a while.
5. **Did one big thing happen here?** A fire, a collapse, a flood. Default: a fire in one corner.
6. **Who comes now, and where do they gather?** Default: FPV pilots, with a fire pit by the main way in.

**From answers to a build list.** The agent turns each answer into three lists in the brief, and then into the Map's script:

| From the answer | Props (shapes in the script) | Decals (one-off marks) | Wear rules (seeded, over whole surfaces) |
|---|---|---|---|
| 1. What it was | Fixings, plinths, rails, signs, an aisle | Floor lines, bay numbers, pale shadows of removed things | Floor wear along the aisle, scuffs at forklift height |
| 2. How it closed | What was left; empty trays; cut bolts | Pipe ghost lines, oil rings | None |
| 3. Wet and old | Fallen sheets and plaster, plants | Streaks under each opening | Moss, damp and rust amount; plant density |
| 4. Who came after | Camp, sorted metal, forced door | Old tags | Litter in corners |
| 5. The big event | The event's remains (burnt timber, collapsed bay) | Soot, scorch | Soot fanning from its source |
| 6. Who comes now | Hangout, seats, litter, launch pad, crashed bits | New tags, a painted mark | Desire paths, litter near seats |

Then the agent places each focal point in the plan, checks the [where things collect](#2-where-things-collect) rules for each, and lists the props with where each goes and what condition it is in, as the Factory's list below does.

## 6. The Factory's story

As the maintainer confirmed it on #290 (24 October 2026): a textile or engineering works, closed, stripped of anything of value, used by scrappers and squatters, fire damage in one corner, then FPV pilots with a fire pit and BBQ spot, bottles, pallet and crate seating, a repaired gate, tags and a few crashed-quad bits.

**The choice made here: an engineering works.** The slice's hall is a single-storey shed with steel trusses, brick pilasters (pillars built into the wall) under each truss and a fallen steel beam, which is how engineering works were built. Textile mills were mostly multi-storey with rows of windows. If the maintainer prefers textile, swap the machine plinths for rows of small loom bolt pads, the crane rail for line-shaft brackets, and add fluff and bobbins to the litter.

**The story, in order.** A light engineering works machined parts here for decades, with an overhead crane down the hall. It closed about 15 years ago and the machines were sold. Thieves stripped the cable, pipes and crane, and knocked through the west wall to drag the last machines out. Then the roof failed: one bay fell in. Squatters camped in the dry north-east corner until their fire got out of hand and burnt the corner. For the last few years, local FPV pilots have used the site. They fixed the yard gate, cleared a hangout inside the loading door, and fly the hall and the fallen bay.

### The hall corner (the slice)

Where things go uses the slice's frame ([PR #317](https://github.com/BartoszSolkaBD/OpenDrone/pull/317), `assets-src/maps/factory/build.py`): the origin is the hall's south-west corner on the floor, east is +x, north is +y, up is +z, and the hall is 30 × 30 m. The sun is in the south-west, 37° up, so the **shaded, damp side is the north and east walls**, and rain falls through the south slope's openings. Things the slice already has are marked *(have)*. The brick pillars built into the long walls under each truss (at x 5, 10, 15, 20 and 25) are the *pilasters*.

**Rules for every prop:**
- Nothing blocks a Gap: the breach (west wall, y ≈ 14.5), the loading door (south wall, x 10.7–14.3), the man door (east wall, y 7–8.1) and the vent (east wall, y 12.1–12.9). The slice's pack test checks lines through them.
- The Launch Spot stays open flat ground with 3 m clear.
- Small props (a bottle, a prop off a quad) are drawn only. Anything a Quad can hit at speed and that is bigger than about 20 cm is solid.
- Vary each copy's turn, tilt, size and wear.

**Focal point A: the fire corner (north-east), squatters and fire.** *(have: burnt timber and ash at x 24, y 24.5; a burnt pallet at x 21, y 20.6)*

| Prop or wear | Where | Condition |
|---|---|---|
| Bed frame with springs | In the ash, x ≈ 25, y ≈ 25.5, along the north wall | Steel, rusted orange-black, sagging; no mattress left |
| Shopping trolley | On its side at the ash's south-west edge, x ≈ 22.5, y ≈ 23 | Blackened, one wheel missing |
| Food tins, a few | In and around the ash | Black, crushed |
| A melted plastic chair | x ≈ 23, y ≈ 26 | Slumped, half black |
| Soot on the walls | North and east walls above the ash, from the floor up to the roof | Black at the bottom, fanning out and fading upwards |
| Soot on the roof | Under the roof and on the trusses over x 21–28, y 21–29 | Sheets and truss underside blackened |
| Scorched plaster | The plaster's east end on the north wall, x 19–20.4, 3–4 m up | Brown scorch, cracked by heat |
| A windbreak | Two pallets standing on edge against the east wall's inside, x ≈ 29.3, y ≈ 20–22 | Charred at the north end, sound at the south |
| Old tags | East wall's inside, y 18–22, 0.8–2 m up | Faded, partly under soot |
| Moss | In the corner's foot, north and east walls, outside the burnt area | Thick and green, as the corner is shaded and damp |

**Focal point B: the pilots' hangout, inside the loading door.** In the light of the open skylight at x 20.9–22.1, y 4–10, so the smoke goes up through it, and close to the way in.

| Prop or wear | Where | Condition |
|---|---|---|
| Fire pit | x ≈ 21.5, y ≈ 6.5, under the skylight opening | A ring of 12–14 loose bricks taken from the rubble, ash in the middle, two half-burnt pallet planks |
| BBQ | x ≈ 19.8, y ≈ 6.8 | A steel grill grate laid on two stacks of 3 bricks; grease-black |
| Cable-drum table | x ≈ 23.2, y ≈ 8.3 | A big wooden cable drum (about 1 m across, 0.6 m tall) on its side, from the works; weathered grey, top stained |
| Pallet benches | Two stacks of 2 pallets in an L, south and east of the fire, about 1.6 m from it | Grey, a few slats broken; one stack newer and paler |
| Crate stools | Three plastic bottle crates, around the fire on the west side | Faded blue and red, one cracked |
| Bottles and cans | 15–25, in a loose ring around the seats, a few on the table, a crate of empties under it | Some crushed; brown and green glass, cans in plain colours (no brands) |
| A bin bag | Against the pilaster at x 20 on the south wall | Full, torn at the bottom |
| Charging spot | On the table: a power bank, a roll of tape, cut zip ties | Fresh |
| A soot fan | On the roof's underside around the skylight opening | Light: these fires are small |
| New tags | South wall's inside, x 16–20, 0.5–2 m up, and on the cable drum | Bright, over one faded older tag |
| Litter along the wall | Against the south wall, x 15–24 | Cans, a crisp packet, leaves |

**Focal point C: the old works and the stripping, along the aisle.** The slice's fallen bay *(have: the heap at x 12.5, y 6.5, fallen sheets and purlin)* and the beam on the floor *(have: x 21–26.5, y 13–17.5)* stay.

| Prop or wear | Where | Condition |
|---|---|---|
| Machine plinths | Three: x 6, y 20 (3 × 1.6 m); x 12, y 22 (2.4 × 1.4 m); x 18, y 22.5 (2 × 1.2 m); 0.15–0.3 m high | Concrete, edges chipped, anchor bolts cut flush (bright cut faces, rusted); oil-black on top and in a ring around |
| Crane rail | On brackets on the pilasters along the north and south walls, 6.6 m up, the full 30 m | Rusty; one cut end hanging down at x ≈ 3 |
| Empty cable tray | Along the north wall at 3.2 m, x 0.5–9.5 | Bent down in one span; a few cut cable ends |
| Fuse board box | East wall, inside, beside the man door at y ≈ 9.5, 1.5 m up | Door open, empty |
| Pipe ghost lines | On the plaster, two pale horizontal lines at 0.4 and 0.6 m with bracket stubs | Pale where the paint was protected |
| Aisle | Walkway lines from the loading door north to y ≈ 26, 3 m wide, running under the rubble heap (the collapse came later) | Paint worn off in the middle, kept at the edges; tyre marks |
| Drag marks | From the plinth at x 6, y 20 to the breach, starting under the breach's rubble | Two scored lines in the floor |
| Scuffs | Loading door jambs and the pilasters near the aisle, 0.3–1.2 m up | Scraped brick and paint |
| Sorted metal pile | x ≈ 3, y ≈ 4, against the south wall | Steel offcuts, a heap of bent conduit, a few aluminium window frames; rusty |
| A filing cabinet | On its side, x ≈ 8, y ≈ 27 | Drawers out and scattered, dented |
| An office chair | x ≈ 9.5, y ≈ 25 | Tipped over, seat torn |
| Bay numbers | On every pilaster, 2.5 m up | Faded stencil, chipped |

**Decay across the hall:**

- Water streaks below every window sill, inside and out; heavier on the south (the weather side), greener on the north (shade).
- Damp patches and puddle stains on the floor under each roof opening, and moss at their edges on the shaded side.
- Rust runs below the truss feet and the crane-rail brackets.
- Efflorescence low on the north and east walls.
- Bird droppings in lines along the trusses at x 5, 10, 15, 20, 25. A nest where the truss at x 25 meets the north wall.
- Leaves and litter drifts in all four corners and against the plinths. The deepest is in the south-east corner, downwind of the breach and the loading door.
- Weeds in floor cracks under the openings *(have, partly)*, and one young birch in the rubble heap.

**Crashed-quad bits.** Few, each where a crash would leave it:
- a whoop's frame caught in the north-west window grid at x 2.5, about 4 m up;
- a 5" prop on the rubble heap, near the top;
- a broken frame arm and an antenna at the foot of the beam on the floor;
- a lost quad on the truss at x 15, over the aisle (seen from below as a shape on the steel).

### The whole site

- **The repaired gate.** The yard's gate in the 6 m opening in the boundary wall's south run (y = −30). It is a double steel gate. One leaf hangs from its hinges with a ratchet strap round the post. The other leaf is propped open with a block. A newer chain and padlock hang on it, bright against the rust. (This is a gate in a wall, which pilots walk through. Where it is open it is a Gap.)
- **The desire path.** Worn through the grass from the gate to the Launch Spot (x 12.5, y −14), then to the loading door: about 1 m wide, bare dirt, softly edged, never a straight line.
- **The launch pad.** A square of plywood with a painted mark, a little off the path near the Launch Spot. If it sits under the Launch Spot itself, the Launch Spot's ground height must be the board's top.
- **The yard:** a skip on its side, rusting, half full of rubble; a car tyre; a pile of pallets the pilots haven't used, grass growing through it; an oil drum.
- **Outside walls:** ivy on the north and east walls; water and algae streaks under each gutter end; tags on the walls you see from the gate.
- **The rest of the works** (the Backdrop's buildings): boarded windows; a faded works name on the largest gable.
- **The other parts of the full Map** (the side buildings, the corridor, the silos and the yard), when they are built: give each one layer that is strongest there. The corridor is the squatters' second camp. The silos carry the high tags (someone climbed). The yard gets the wreck of the works' van.

## Sources

Links only. Nothing is copied from them.

- Harvey Smith and Matthias Worch, [What Happened Here? Environmental Storytelling](https://gdcvault.com/play/1012647/What-Happened-Here-Environmental), GDC 2010: telling a story through a room's contents and the traces of what happened.
- [Urban exploration](https://en.wikipedia.org/wiki/Urban_exploration) (Wikipedia): the hobby and photography of abandoned places.
- [Metal theft](https://en.wikipedia.org/wiki/Metal_theft) (Wikipedia): what is taken from empty buildings, and why copper goes first.
- [Metal theft from abandoned buildings creates costly problems](https://www.businessinsurance.com/article/00010101/NEWS06/302059991/Metal-theft-from-abandoned-buildings-creates-costly-problems) (Business Insurance): the stripping, and the water damage that follows.
- [Efflorescence](https://en.wikipedia.org/wiki/Efflorescence) and [Spalling](https://en.wikipedia.org/wiki/Spalling) (Wikipedia): salt bloom on wet masonry, and concrete flaking.
- [Ruderal species](https://en.wikipedia.org/wiki/Ruderal_species) and [Buddleja davidii](https://en.wikipedia.org/wiki/Buddleja_davidii) (Wikipedia): the plants that colonise rubble and derelict sites first.
- [Desire path](https://en.wikipedia.org/wiki/Desire_path) (Wikipedia): paths worn by people taking the shortest route.
- [Squatting](https://en.wikipedia.org/wiki/Squatting) (Wikipedia): living in empty buildings.
- [Introduction to the world of FPV drones](https://www.421.news/en/introduction-to-fpv-drones/) (421 News): the freestyle community and its abandoned places ("bandos").
- [Back to the abandoned factory](https://intofpv.com/t-back-to-the-abandoned-factory) (IntoFPV forum): pilots on flying an abandoned factory.
