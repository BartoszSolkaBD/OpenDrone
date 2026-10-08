# Feel Test log: Whoop 65

Every Estimate a Feel Test moves in this Quad's `quad.toml` gets one row here, oldest first: the date, the number as `[section] key`, its old and new values exactly as `quad.toml` writes them, and why. The new value stays inside the Estimate's range, and rows are never changed or removed. CI checks this log against every change to `quad.toml`; see [Checking a Pack](../../../../docs/verification/checking-a-pack.md).

| Date | Number | Old → new | Why |
|---|---|---|---|
| 2026-10-08 | [frame] drag_area | front 9, side 9, top 25 cm² → front 15, side 17, top 17 cm² | New source: worked out from the collision shapes (#42), in place of #16's first guess; each is the silhouette of the frame and canopy, the pack and the duct rings seen from the front, the side and above, times a drag coefficient of 1, leaving out the prop discs from above (see docs/research/quad-definitions.md) |
