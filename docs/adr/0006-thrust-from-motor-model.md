# Thrust comes from a motor model, not a fitted thrust curve

A Quad's thrust comes from a motor model. Battery voltage, the motor's KV, winding resistance and no-load current, and the prop's coefficients together set each motor's speed, and thrust and torque grow with the square of that speed. We rejected fitting Betaflight's ArduPilot-style thrust curve (`(1 − e)·m + e·m²`) to thrust tables and scaling it by voltage. The reference-data research had suggested it so that a pilot's `thrust_linear` setting would carry over. But that setting belongs to the Flight Controller and works either way, and a fitted curve would need battery sag, climb and descent bolted on separately, so they wouldn't interact the way they do in the air. With the motor model, fading punch, idle drop on a tired pack and current draw all follow without extra code. Settled in [#10](https://github.com/BartoszSolkaBD/OpenDrone/issues/10).

## Consequences

- A Quad definition carries motor and prop numbers (KV, winding resistance, no-load current, pole count, prop coefficients, spin-up and slow-down times), not a thrust curve.
- Thrust Stand Scenarios check the model against the manufacturer's thrust tables (BetaFPV for the whoop, T-Motor for the 5"). Betaflight's curve formula becomes a check, not the model.
- No whoop motor's winding resistance is published, so the whoop's starts as an Estimate and is tuned in Feel Tests.
