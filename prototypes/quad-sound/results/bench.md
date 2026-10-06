# Sound bench (PROTOTYPE, #34)

Machine: Apple M4

## CPU cost, offline (whole graph: 4 motor voices, listener, volumes, samplers)

| Quad | Listening Position | % of one core |
|---|---|---|
| Whoop 65 (Meteor65 Pro) | On the Quad | 1.12% |
| Whoop 65 (Meteor65 Pro) | Where you stand | 1.14% |
| Freestyle 5" | On the Quad | 1.30% |
| Freestyle 5" | Where you stand | 1.32% |

## Live stream on the default output device (master volume 0, so nothing is heard)

Output delay is what CoreAudio reports through cpal: the device buffer plus the device's latency and safety offset. It is not measured acoustically (the Mac mini has no microphone). A new tick can wait up to one block before the next callback reads it, so the worst case adds one block; the average adds half. The game adds its own frame-to-sound hand-off on top (not in this prototype: here the simulation thread publishes every 1 ms).

| Requested block | Block in use | Sample rate | Output delay (callback to speaker) | Worst case tick to speaker (+ one block wait) | Tick age at read | Peak CPU in a block | Device |
|---|---|---|---|---|---|---|---|
| 64 | 64 frames (1.3 ms) | 48000 Hz | 3.5 ms | 4.9 ms | 0.67 ms | 9.9% | External Headphones |
| 128 | 128 frames (2.7 ms) | 48000 Hz | 4.9 ms | 7.5 ms | 0.67 ms | 6.7% | External Headphones |
| 256 | 256 frames (5.3 ms) | 48000 Hz | 7.5 ms | 12.9 ms | 0.65 ms | 4.9% | External Headphones |
| 512 | 512 frames (10.7 ms) | 48000 Hz | 12.9 ms | 23.5 ms | 0.62 ms | 10.6% | External Headphones |
| 1024 | 1024 frames (21.3 ms) | 48000 Hz | 23.5 ms | 44.9 ms | 0.60 ms | 9.9% | External Headphones |
| device default | 1024 frames (21.3 ms) | 48000 Hz | 23.5 ms | 44.9 ms | 0.60 ms | 9.9% | External Headphones |
