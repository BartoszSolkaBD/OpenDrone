# Feel Test log: Freestyle 5″

Nobody Feel-Tests the Freestyle 5″: its Estimates are fitted to public flight logs instead (#10 §7). Every Estimate that moves in this Quad's `quad.toml` still gets one row here, oldest first: the date, the number as `[section] key`, its old and new values exactly as `quad.toml` writes them, and why. The new value stays inside the Estimate's range, and rows are never changed or removed. CI checks this log against every change to `quad.toml`; see [Checking a Pack](../../../../docs/verification/checking-a-pack.md).

| Date | Number | Old → new | Why |
|---|---|---|---|
| 2026-10-07 | [props] power_coefficient | 0.09 → 0.105 | fitted to T-Motor's T5147 table through the motor model (#41): at full drive from its 24.0 V supply it now makes about 1,580 g at 29,500 RPM and 35.0 A, against the table's 1,591 g, 29,447 RPM and 34.6 A; 0.09 drew 14% too little current at full throttle |
| 2026-10-07 | [motors] winding_resistance | 0.20 Ω → 0.19 Ω | fitted with the power coefficient to T-Motor's full-throttle row (#41): 0.19 Ω turns the motor at the table's 29,447 RPM from 23.5 V at full drive |
