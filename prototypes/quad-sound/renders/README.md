# Rendered sounds (PROTOTYPE, #34)

Made offline by `./run.sh render` with the tuning in effect, through the same Firewheel graph the app plays live. 48 kHz, 16-bit stereo WAV here (not committed); AAC 256 kbps copies with the same names in `m4a/` (committed, so they can be played from the branch). Peak is of the whole file (1.0 = full scale). Use headphones or decent speakers; a laptop speaker loses the low end.

| File | Quad | Listening Position | What happens | Peak |
|---|---|---|---|---|
| `01-whoop-powerup-onquad.wav` | Whoop 65 (Meteor65 Pro) | On the Quad | Reset: Bluejay start-up melody and ready beeps, arm, idle, disarm | 0.32 |
| `02-whoop-hover-onquad.wav` | Whoop 65 (Meteor65 Pro) | On the Quad | take off, hover with small corrections, land | 0.36 |
| `03-whoop-punchout-onquad.wav` | Whoop 65 (Meteor65 Pro) | On the Quad | hover, full throttle 1.6 s, chop, catch | 0.46 |
| `04-whoop-punchout-standing.wav` | Whoop 65 (Meteor65 Pro) | Where you stand | the same flight heard from the Launch Spot | 0.33 |
| `05-five-powerup-onquad.wav` | Freestyle 5" | On the Quad | Reset with the 5": ESC melody plus Betaflight's buzzer, arm and disarm beeps | 0.24 |
| `06-five-hover-onquad.wav` | Freestyle 5" | On the Quad | take off, hover, land | 0.24 |
| `07-five-punchout-onquad.wav` | Freestyle 5" | On the Quad | hover, full throttle 1.6 s, chop, catch | 0.51 |
| `08-five-punchout-standing.wav` | Freestyle 5" | Where you stand | the same flight heard from the Launch Spot | 0.69 |
| `09-five-punchout-onquad-clipping.wav` | Freestyle 5" | On the Quad + clipping | as 07, with clipping on | 0.32 |
| `10-five-flip-onquad.wav` | Freestyle 5" | On the Quad | punch, acro roll flip, catch | 0.49 |
| `11-five-propwash-onquad.wav` | Freestyle 5" | On the Quad | chop and fall into your own air: motors warble | 0.38 |
| `12-five-flyby-standing.wav` | Freestyle 5" | Where you stand | fly-by at 26 m/s, 8 m in front: Doppler | 0.26 |
| `13-five-around-tower-standing.wav` | Freestyle 5" | Where you stand | round the Bando tower: muffled behind it | 0.50 |
| `14-five-outandback-standing.wav` | Freestyle 5" | Where you stand | out to 60 m and back: fade, delay | 0.38 |
| `15-five-propstrike-onquad.wav` | Freestyle 5" | On the Quad | two Prop Strikes: ticks, motor recovers | 0.25 |
| `16-five-jam-onquad.wav` | Freestyle 5" | On the Quad | jammed prop: restarts fail, three falling ESC tones | 0.23 |
| `17-five-crash-onquad.wav` | Freestyle 5" | On the Quad | dive into the ground: hit clip | 0.76 |
| `18-five-failsafe-onquad.wav` | Freestyle 5" | On the Quad | link lost, drop after 1.5 s, RX_LOST beeps until the link returns | 0.77 |
| `19-five-lowbattery-onquad.wav` | Freestyle 5" | On the Quad | pack sags: LOW BATTERY then LAND NOW beeps | 0.30 |
| `20-whoop-crash-onquad.wav` | Whoop 65 (Meteor65 Pro) | On the Quad | whoop dives into the ground: hit clip | 0.36 |
| `21-five-intobowl-standing.wav` | Freestyle 5" | Where you stand | down into the Skate Park's pool bowl: muffled by its walls | 0.09 |
| `22-whoop-flyby-standing.wav` | Whoop 65 (Meteor65 Pro) | Where you stand | whoop fly-by at 11 m/s, 8 m in front | 0.09 |
| `23-whoop-beacon-onquad.wav` | Whoop 65 (Meteor65 Pro) | On the Quad | 10 idle minutes skipped: Bluejay's beacon every 3.1 s | 0.76 |
| `24-five-pausemenu-onquad.wav` | Freestyle 5" | On the Quad | Pause Menu at 2.5 s: Quad silent, background dips, menu click; Resume at 5 s | 0.22 |

Whole graph, offline: 1.31% of one M4 core on average (196.0 s of sound in 2.576 s).
