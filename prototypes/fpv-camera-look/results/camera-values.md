# PROTOTYPE #28 round 3: each camera's dynamic range and resolution

All sources were read on 2026-10-04. No published lab measurement of dynamic range (DR) or
resolution exists for any of these FPV cameras, so every DR value is an Estimate, bounded by the
manufacturer's signal-to-noise (S/N) figure (stops = dB / 6.02; S/N is an upper limit, not DR).
TVL is measured per picture height: lines across a 4:3 picture = TVL × 4/3.

## Values the prototype uses (Analog: future `[camera]` keys per Quad; Digital: one generic set on the Video Look)

| Quad | Key | Value | Confidence | Range | Source |
|---|---|---|---|---|---|
| Whoop 65 (BetaFPV C03) | analog_dynamic_range | 7 stops | Estimate | 6–8.5 stops | c03 (S/N >50 dB, about 8.3 stops at most; D-WDR on a 1/3" sensor) |
| Whoop 65 | analog_lines | 480 lines | Manufacturer | — | c03 (NTSC 4:3 only), ntsc |
| Whoop 65 | analog_sharpness | 300 TVL | Estimate | 250–380 TVL | ntsc (composite NTSC carries about 330 TVL; the "1200TVL" claim can't pass it) |
| Freestyle 5" (Ratel 2 / Phoenix 2 class) | analog_dynamic_range | 8.5 stops | Estimate | 7–10 stops | ratel-pro (S/N >60 dB), phoenix2 (S/N >50 dB) |
| Freestyle 5" | analog_lines | 480 lines | Manufacturer | — | ntsc (NTSC default; PAL 576 possible) |
| Freestyle 5" | analog_sharpness | 400 TVL | Estimate | 330–480 TVL | ntsc, rec601 (hard limit 540 TVL) |
| Generic Digital look (not per Quad) | dynamic_range | 10 stops | Estimate | 9–12 stops | o4, walksnail (no figure published; live view uses normal colour, not Log) |
| Generic Digital look | lines | 1080 lines | Manufacturer | — | o4 (1080p at up to 100 fps), walksnail (1080p60) |

Interlaced analog resolves about 70% of its lines (Kell factor, Derived): the prototype blurs
vertically by 1/0.7 of a line.

## Findings behind them

- **Meteor65 Pro (2022):** ships the C03: 1/3" CMOS, "1200TVL with Global WDR", D-WDR auto,
  NTSC 4:3 only, S/N >50 dB, 160°, 2.1 mm M7 lens. The newer Meteor65 Pro II ships a different
  "Air Camera" (1/4.5", 600 TVL, S/N 40.59 dB ≈ 6.7 stops).
- **Cetus X:** ships the C04 (1/3", 1200 TVL claimed, NTSC), "based on Caddx Nano Ant / RunCam
  Nano 4" (Nano 4: 800 TVL, S/N >50 dB).
- **5" analog:** Caddx Ratel 2 (1/1.8", 1200 TVL, Super WDR, no dB), Caddx Ratel Pro (1/1.8" BSI,
  1500 TVL, S/N >60 dB), RunCam Phoenix 2 (1/2", 1000 TVL, S/N >50 dB), Foxeer Razer Mini
  ("WDR 90 dB", a sensor claim that doesn't reach the goggles).
- **Analog chain:** NTSC shows 480 lines (PAL 576); composite NTSC carries about 330 TVL
  (broadcast figure), PAL D about 470; DVRs digitise at 720 samples a line (540 TVL at most).
- **Digital:** DJI O4 Pro: 1/1.3", live view 1080p at up to 100 fps, records 10-bit D-Log M;
  DJI O4 (Lite): 1/2", 1080p up to 100 fps, "lower dynamic range" than the Pro (Oscar Liang);
  DJI O3: 1/1.7", 1080p100; Walksnail Avatar HD Pro: 1/1.8" Starvis II, 1080p60 or 720p120.

## Sources

- c03: https://betafpv.com/products/c03-fpv-micro-camera
- meteor65pro: https://racedayquads.com/products/betafpv-2022-bnf-meteor65-pro-1s-brushless-whoop-elrs-2-4ghz
- meteor65pro2: https://betafpv.com/products/meteor65-pro-ii-brushless-whoop-quadcopter ; https://betafpv.com/products/air-camera
- cetusx: https://betafpv.com/products/cetus-x-brushless-quadcopter ; https://betafpv.com/products/c04-fpv-camera ; https://shop.runcam.com/runcam-nano-4/
- ratel2: https://racedayquads.com/products/caddx-ratel-2-1200tvl-16-9-4-3-ntsc-pal-micro-fpv-camera
- ratel-pro: https://caddxfpv.com/products/caddxfpv-ratel-pro-analog-camera
- phoenix2: https://shop.runcam.com/runcam-phoenix-2/
- razer: https://www.foxeer.com/foxeer-razer-mini-v2-fpv-camera-g-569
- ntsc: https://en.wikipedia.org/wiki/Television_lines ; https://en.wikipedia.org/wiki/480i ; https://en.wikipedia.org/wiki/Kell_factor
- rec601: https://en.wikipedia.org/wiki/Rec._601
- o4: https://www.dji.com/o4-air-unit/specs ; https://oscarliang.com/dji-o4-air-unit/ ; https://oscarliang.com/dji-o4-air-unit-pro/
- o3: https://oscarliang.com/dji-o3-air-unit/
- walksnail: https://caddxfpv.com/products/walksnail-avatar-hd-pro-kit
