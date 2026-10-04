# Camera and video

What the pilot sees through the Quad's FPV Camera, and how its video reaches them. Back to the [map](../../CONTEXT.md).

## Language

**FPV Camera**:
The camera fixed to the Quad's frame that the pilot flies through. In the alpha it is the only view.
_Avoid_: Cam, camera (on its own, once other views exist)

**Camera Tilt**:
How far the FPV Camera points up from the Quad's frame. Each Quad has a default, and the pilot can change it on every Quad.
_Avoid_: Camera angle, angle (Angle is a Flight Mode), uptilt

**Lens**:
The fisheye shape of a Quad's FPV Camera, set by the Quad's definition. Straight lines bend towards the edges, as on a real FPV camera.
_Avoid_: Fisheye (as a setting name), distortion, lens FX

**FOV**:
How wide the FPV Camera sees, in degrees, measured corner to corner of the 4:3 picture, like the number on a camera's box. Each Quad has a default, and the pilot can change it on every Quad.
_Avoid_: Zoom, horizontal FOV (other sims' numbers are measured across a flat lens and don't compare)

**Video Look**:
How the pilot's picture is made: Analog or Digital. It belongs to the pilot, not to the Quad.
_Avoid_: Video mode, filter, VTX type, video system

**Video Signal**:
How strongly the Quad's video reaches the pilot. It depends on the Quad's VTX power, its distance from the pilot and how much material lies between them. It is separate from the Radio Link, which carries control.
_Avoid_: Signal (on its own), RSSI, reception, link

**Breakup**:
The picture falling apart as the Video Signal weakens: static, colour loss and tearing in Analog; smearing, stutter and freezes in Digital. The pilot's Breakup setting is Off, Light, Medium or Realistic.
_Avoid_: Analog noise, noise, static, interference (as the setting's name)

**Dynamic Range**:
How many stops of light the FPV Camera's live picture holds between black and white. Anything brighter blows out to white and anything darker sinks to black. Each Quad's analog camera has its own, and Digital has one for every Quad.
_Avoid_: WDR, HDR (those are camera makers' claims, not what reaches the goggles)

**Auto-exposure**:
The FPV Camera brightening or darkening its picture as the light changes, within limits and over about a second, as a real camera does. Flying into a dark room starts near black with the windows blown out; flying back out blows out before the picture recovers.
_Avoid_: Eye adaptation, auto-brightness

## Rules

- Camera settings never touch the physics, so they can change mid-flight.
- Nothing about the camera or the video enters the Simulation: not the Video Signal, not Breakup, not Digital's extra delay. Breakup never touches the physics or the Radio Link, so the pilot keeps control in full static.
- The FPV Camera is bolted to the frame. It moves exactly as the physics moves the Quad, with no added shake.
- The pilot stands at the Map's Launch Spot, and the Video Signal is received there at head height. Reset moves the Quad, never the pilot.
- Every solid part of a Map weakens the Video Signal, ground and floors included, by how much of it the signal passes through. Each wall counts at most 0.35 m of concrete.
- Walls are averaged across the width of the radio wave, about 0.6 m around the Quad, and the received level follows changes over about 0.3 s. So going behind an edge or through a doorway fades the picture over about a metre: Breakup builds up gradually, and the pilot can feel it coming and turn back ([ADR-0019](../adr/0019-video-signal-averages-walls-over-the-wave-width.md)).
- Each Quad's analog FPV Camera has its own Dynamic Range, lines top to bottom and sharpness across (in TV lines), taken from its real camera. Digital is one generic camera for every Quad, set with the Video Look.
- Interlaced analog video resolves only about 70% of its lines, so the Analog picture is softened vertically to match.
- Analog's soft picture, grain and colour bleed belong to the Video Look, not to Breakup, so they stay with Breakup Off.
- Digital always shows the picture a little later than Analog, as real digital systems do. The delay is in the picture only, never in the sticks.
- On Light the picture is never lost completely. With Reduce motion on, Breakup flashes at most 3 times a second.
- In Analog, Betaflight's OSD rides the video, so Breakup hits it too. In Digital the OSD stays clean. The sim's own notices always sit clean on top of the video.
- Graphics quality tiers may change how sharp the picture is, never the Lens, FOV, Breakup or Digital's delay. Every machine shows the same signal.
- All camera and video effects together fit in 2 ms of a High-tier frame on the dev machine, and the Map is drawn once per frame ([ADR-0009](../adr/0009-fisheye-from-one-warped-render.md)).
- Only the camera's own steps count against those 2 ms: the merged pass and what feeds it. Drawing the Map wider than the picture, for the fisheye, counts as drawing the Map, under the 45 fps promise.
- Shade sits about 2–3 stops below sun and the inside of a building about 5–6 stops below. Auto-exposure needs those ratios to look right, so a Map's lighting must keep them.
