# The Feel Test checklist

A Feel Test is a session in which a pilot who flies the real Quad flies the simulated one against their memory of it, following this checklist ([Verification](context/verification.md)). Scenarios prove what numbers can; a Feel Test judges what only a pilot can, and it's never run in CI.

In the alpha, the maintainer Feel-Tests the **Whoop 65** against their real BetaFPV Meteor65 Pro ([#10 §7](https://github.com/BartoszSolkaBD/OpenDrone/issues/10)). The **Freestyle 5″** isn't Feel-Tested: it's checked by data alone, with rule Scenarios and comparisons with the TII flight logs, and flown only as a sanity check.

## The rules

- **A Feel Test moves only Estimates, and only inside their ranges.** Measured, Manufacturer and Derived numbers are locked. If only a locked number would fix a feel, an effect is missing or wrong, and that's what gets fixed ([Flying](context/flying.md)).
- **Every change is logged** in the Quad's Feel Test log, `feel-tests.md` in its Pack folder: the date, the number, its old and new value, and why. CI blocks a change to an Estimate that has no log row, or that leaves the Estimate's range.
- **A signed-off feel is pinned.** Once the maintainer signs off a feel, the tuned numbers and the behaviour they give become Observed Expectations in Scenarios.
- A `build` label on a pull request builds the game from it, so a change can be Feel-Tested before it merges.

## The whoop's ten manoeuvres

| | Manoeuvre | What to compare with the real whoop |
|---|---|---|
| 1 | Hover and float on a fresh pack, then skim the floor | The throttle it takes to hover (about 40–50% on the real Meteor65 Pro), how it floats, and how it cushions just above the floor |
| 2 | Punch-outs from a hover | How fast the motors spool up and the whoop climbs, how far the voltage sags, and the small yaw twitch as it goes |
| 3 | Flips and rolls on each axis | How quickly it starts and stops rotating, and how cleanly it holds its attitude afterwards |
| 4 | A sprint, then let go | Whether it carves rather than slides, how its nose lifts at speed, and how quickly it slows down when you let go |
| 5 | Chop-and-dive, power loop and split-S | The Prop Wash as you catch the dive: where it starts, how hard it shakes, and that flying forward out of your own air cures it |
| 6 | Fly up to the Bando ceiling | The pull towards the ceiling under Bando's 3.1 m ceilings |
| 7 | Bump the walls | How it bounces and slides off a wall, and how hard a prop that touches grabs |
| 8 | One pack from full to empty | How punch fades and idle drops as the pack tires, and how a flat pack fades until the whoop can't hover, with no cutoff |
| 9 | Land upside down on a hard floor and Crash Flip back, on roll, on pitch and on a diagonal | How readily it flips back over, and how long it takes |
| 10 | Clip an edge or a rail with the open top of a duct | How a prop that touches brakes its motor and kicks the whoop |

Manoeuvres 1 to 8 come from [#10 §7](https://github.com/BartoszSolkaBD/OpenDrone/issues/10), and 9 and 10 from [#26](https://github.com/BartoszSolkaBD/OpenDrone/issues/26), which also made manoeuvre 7 tune prop grip.

## The numbers set by feel

These Estimates were left to Feel Tests when they were decided. Any other Estimate may move too, inside its range, when a manoeuvre points to it.

- **Bounce and friction,** how the Quad comes off the Map ([#10](https://github.com/BartoszSolkaBD/OpenDrone/issues/10)): manoeuvre 7.
- **Prop grip,** how hard a spinning prop grabs what it touches ([#26](https://github.com/BartoszSolkaBD/OpenDrone/issues/26)): manoeuvres 7 and 10. The whoop's is tuned by feel; the 5″'s stays put.
- **Prop Wash strength and flicker speed** ([ADR-0005](adr/0005-prop-wash-from-physics.md)): manoeuvre 5.
- **The ground-effect body term** ([#16](https://github.com/BartoszSolkaBD/OpenDrone/issues/16)): manoeuvre 1.

Each Quad's log is in [Feel Test logs](book/feel-test-logs.md).
