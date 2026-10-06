# The Video Signal averages walls over the radio wave's width, not along a thin line

To work out how many walls lie between the Quad and the pilot, the Video Signal asks the Simulation "which Map surfaces does this line pass through?" for the straight line and for 8 more lines around it, spread about 0.6 m around the Quad. It averages the power that gets through all 9 (in milliwatts, not decibels), and then lets the received level follow changes over about 0.3 s. Each wall counts at most 0.35 m of concrete. A radio wave at 5.8 GHz is about this wide over these distances (its first Fresnel zone), so partly blocked paths lose part of the signal, as they do in real flight.

We chose this because a single thin line made Breakup snap: flying past a doorway or a floor edge switched the picture from clean to full static in one frame, with no warning. In the FPV camera prototype, on the Bando's shaft path on Realistic, 18% of the flight had no picture and the warning was 0.1 s. With the averaging, the picture now fades over about a metre and the pilot can turn back; the maintainer confirmed it by flying. Decided in [#28](https://github.com/BartoszSolkaBD/OpenDrone/issues/28).

## Considered options

- **One straight line (the first version).** Cheapest, but Breakup switches on and off at wall edges, and a slanted line through a floor slab could count a metre of concrete.
- **Only a time delay on the level.** Softens the jump into a fixed fade, but the fade starts only after the Quad is already behind the wall, so the pilot still gets no warning.
- **Real reflections (multipath).** Out of the alpha, per [#14](https://github.com/BartoszSolkaBD/OpenDrone/issues/14).

## Consequences

- The signal check asks 9 line questions instead of 1. In the prototype that took microseconds a frame on the Bando's collision shapes.
- Breakup near walls depends on the spread and the smoothing time. Both are tuning values, not physics, and never enter the Simulation ([ADR-0003](0003-crate-split-and-flight-inputs.md)).
- The readable checks for the Video Signal (#14's examples) must use the same averaging, or their numbers won't match the game.
