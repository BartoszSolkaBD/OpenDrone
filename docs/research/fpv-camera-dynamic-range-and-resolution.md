# FPV cameras: dynamic range and resolution

Research for [#28](https://github.com/BartoszSolkaBD/OpenDrone/issues/28), to give each FPV Camera its real limits: how many stops of light its live picture holds (Dynamic Range) and how sharp that picture is. All sources were read on 2026-10-04.

## What exists, and what doesn't

- **No lab measurement exists** of dynamic range or resolution for any of these FPV cameras. We looked for CineD lab tests and for Oscar Liang's and Joshua Bardwell's reviews. So every dynamic-range value below is an Estimate.
- **What makers publish instead:**
  - **Signal-to-noise (S/N), in dB.** It caps the dynamic range: stops = dB ÷ 6.02. It is an upper limit, not the dynamic range itself.
  - **"WDR" or "HDR" claims.** These describe the sensor or its processing, not what reaches the goggles.
- **TVL** is measured per picture height. A 4:3 picture's lines across = TVL × 4/3.
- **Analog cameras' TVL claims (1000–1500 TVL) can't pass the video signal.** NTSC composite carries about 330 TVL, PAL about 470. A goggle or video recorder samples 720 points a line, a hard limit of 540 TVL. Carrying a real 1200 TVL would need about 15 MHz of video bandwidth (Derived).

## The cameras

**Whoop: Meteor65 Pro (2022), BetaFPV C03.**
- **What it is:** the "upgraded C03" camera, with a 1/3" CMOS sensor. BetaFPV doesn't publish the sensor model or its pixel count.
- **Claims:** "1200TVL with Global WDR", D-WDR on automatic.
- **Video:** NTSC 4:3 only.
- **S/N:** more than 50 dB, so at most about 8.3 stops.
- **Lens:** 160°, 2.1 mm, M7.
- **Not this camera:** the newer Meteor65 Pro II ships an "Air Camera" (1/4.5", 600 TVL, S/N 40.59 dB, about 6.7 stops).

Sources: [BetaFPV C03](https://betafpv.com/products/c03-fpv-micro-camera), [Meteor65 Pro listing](https://racedayquads.com/products/betafpv-2022-bnf-meteor65-pro-1s-brushless-whoop-elrs-2-4ghz), [Air Camera](https://betafpv.com/products/air-camera).

**Cetus X: BetaFPV C04.**
- 1/3", 1200 TVL claimed, NTSC.
- BetaFPV says it is based on the RunCam Nano 4 or Caddx Ant. The Nano 4 is 800 TVL with S/N above 50 dB.

Sources: [Cetus X](https://betafpv.com/products/cetus-x-brushless-quadcopter), [C04](https://betafpv.com/products/c04-fpv-camera), [RunCam Nano 4](https://shop.runcam.com/runcam-nano-4/).

**Typical 5" analog cameras.**

| Camera | Sensor | TVL claimed | What the maker says about range |
|---|---|---|---|
| Caddx Ratel 2 | 1/1.8" | 1200 | "Super WDR", no figure |
| Caddx Ratel Pro | 1/1.8" BSI | 1500 | S/N above 60 dB, about 10 stops at most |
| RunCam Phoenix 2 | 1/2" | 1000 | S/N above 50 dB, about 8.3 stops at most |
| Foxeer Razer Mini | 1/3" | 1200 | "WDR 90 dB", a sensor claim that doesn't reach the goggles |

Sources: [Ratel 2](https://racedayquads.com/products/caddx-ratel-2-1200tvl-16-9-4-3-ntsc-pal-micro-fpv-camera), [Ratel Pro](https://caddxfpv.com/products/caddxfpv-ratel-pro-analog-camera), [Phoenix 2](https://shop.runcam.com/runcam-phoenix-2/), [Razer Mini](https://www.foxeer.com/foxeer-razer-mini-v2-fpv-camera-g-569).

**Digital systems (references only; the Digital look carries no brand).**

| System | Sensor | Live picture sent to the goggles |
|---|---|---|
| DJI O4 Pro | 1/1.3" | 1080p at up to 100 fps |
| DJI O4 | 1/2" | 1080p at up to 100 fps |
| DJI O3 | 1/1.7" | 1080p100 |
| Walksnail Avatar HD Pro | 1/1.8" | 1080p60 or 720p120 |

- None publishes a dynamic range.
- The live picture uses normal colour, not the flat Log colour used for recordings.
- Oscar Liang notes the O4 has "lower dynamic range" than the O4 Pro.

Sources: [DJI O4 specs](https://www.dji.com/o4-air-unit/specs), [O4 review](https://oscarliang.com/dji-o4-air-unit/), [O4 Pro review](https://oscarliang.com/dji-o4-air-unit-pro/), [O3 review](https://oscarliang.com/dji-o3-air-unit/), [Walksnail Avatar HD Pro](https://caddxfpv.com/products/walksnail-avatar-hd-pro-kit).

**The analog video signal.**
- NTSC shows 480 lines and PAL 576.
- Interlaced video resolves only about 70% of its lines (the Kell factor).

Sources: [Television lines](https://en.wikipedia.org/wiki/Television_lines), [480i](https://en.wikipedia.org/wiki/480i), [Kell factor](https://en.wikipedia.org/wiki/Kell_factor), [Rec. 601](https://en.wikipedia.org/wiki/Rec._601).

## Values chosen

The maintainer confirmed these by eye in the prototype. They become `[camera]` keys in each Quad definition; Digital's are one generic set, kept with the Video Look.

| Camera | Dynamic Range | Lines | Sharpness across |
|---|---|---|---|
| Whoop 65 (C03) | 7 stops, Estimate, 6–8.5 | 480, Manufacturer (NTSC) | 300 TVL, Estimate, 250–380 |
| Freestyle 5" (typical camera) | 8.5 stops, Estimate, 7–10 | 480, Manufacturer (NTSC) | 400 TVL, Estimate, 330–480 |
| Digital (generic, every Quad) | 10 stops, Estimate, 9–12 | 1080, Manufacturer | the transmitted 1080p picture |

The sharpness Estimates could later be checked by filming a resolution chart through the goggles' video recorder.
