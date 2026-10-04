# The FPV Camera's fisheye comes from one warped render

Real FPV lenses are fisheyes: the Meteor65 Pro's C03 camera is 160° corner to corner, about 128° across. A normal game camera can't show that, because its edges stretch four times over at 60° off centre and it can never reach 180°. So we draw the Map once with a normal camera, wider than the picture, and warp it into the Quad's Lens in the same single pass that makes the Video Look. The price is a softer centre at wide FOV. At 160° the middle of the picture holds about a quarter of native 1440p's detail across, roughly 470 pixels. Analog hides that, because a real analog picture is about as soft (NTSC carries about 440). We accepted it to keep every camera and video effect within 2 ms of the 22 ms frame on the Mac mini M4, and to keep phones possible. Decided in [#14](https://github.com/BartoszSolkaBD/OpenDrone/issues/14).

## Considered options

- **A multi-camera "true lens", as in Velocidrone.** Sharp everywhere, but it draws the Map 3 to 5 times a frame, and Bevy keeps a set of shadow maps for every view. It is very unlikely to hold 45 fps in the Bando on the M4, and it is too heavy for phones.
- **A plain game camera (rectilinear).** Sharpest and cheapest, but the edges stretch, straight lines stay straight, and it can't reach real FPV fields of view. It doesn't look like FPV.
- **Bending the Map's geometry in the vertex shader.** It needs very dense meshes, because wgpu has no tessellation, and it breaks Bevy's culling.

## Consequences

- The centre softens as FOV widens, and the widest FOVs (up to 170°) look soft.
- Digital is the look that shows it. Where the 2 ms cap and 45 fps allow, Digital may draw the Map larger than the screen and use a gentler lens curve. The FPV camera prototype decides by eye.
  - Settled in [#28](https://github.com/BartoszSolkaBD/OpenDrone/issues/28): Digital uses a gentler lens curve and the Map stays drawn at the picture's own size. Drawing it 1.5× larger would have added about 2.2 ms a frame.
- Don't fix the soft centre by drawing the Map more than once. Reopen this decision first.
