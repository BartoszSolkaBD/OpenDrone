# Rendered sounds (PROTOTYPE, #34)

Made offline by `./run.sh render` with the tuning in effect, through the same Firewheel graph the app plays live. 48 kHz, 16-bit stereo WAV here (not committed); AAC 256 kbps copies with the same names in `m4a/` (committed, so they can be played from the branch). Peak is of the whole file (1.0 = full scale). Use headphones or decent speakers; a laptop speaker loses the low end.

| File | Quad | Listening Position | What happens | Peak |
|---|---|---|---|---|
| `02-whoop-hover-onquad.wav` | Whoop 65 (Meteor65 Pro) | On the Quad | take off, hover with small corrections, land | 0.48 |

Whole graph, offline: 1.37% of one M4 core on average (10.0 s of sound in 0.137 s).

## Round 1, for comparison

`m4a/round1/` keeps five round-1 files: 02, 03, 06 and 07 (the old voice), and 13 (round the tower, where it went silent).
