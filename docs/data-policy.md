# The data policy

What data may enter the repo, where the data we may not commit lives, and how credit is kept. It covers everything that isn't code: reference flight logs and datasets, thrust tables, textures, sounds, fonts, recordings and settings exports. Libraries have their own rules, in [ADR-0014](adr/0014-licences-for-libraries-and-assets.md) and [`deny.toml`](../deny.toml).

The short version: **only CC0 assets and CC BY data enter the repo.** Data with no licence is downloaded when it's needed and never committed ([ADR-0014](adr/0014-licences-for-libraries-and-assets.md), settled for data in [#11](https://github.com/BartoszSolkaBD/OpenDrone/issues/11)).

## What may be committed

- **Assets OpenDrone's agents make,** such as Maps, Quad models, baked lighting and renders, dedicated to the public domain under CC0. The scripts that make them are code, under MIT or Apache-2.0.
- **Outside assets under CC0,** such as textures from Poly Haven and ambientCG, and the hit clips, menu clips and Background Sounds. An outside font may also be under the SIL Open Font Licence.
- **Outside data under CC BY 4.0, with its credit,** such as the flight logs of TII's *Race Against the Machine* dataset: committed only as trimmed CSV files, with an attribution notice beside them.
- **The maintainer's own exports and recordings,** such as the Betaflight `diff all` exports in [`research/quad-settings/`](research/quad-settings/README.md), with anything that identifies a device removed first: the ExpressLRS UID and the board's MCU id.

## What is never committed

- **Data with no licence.** No licence means all rights reserved, even when the data is public: for example NeuroBEM, the IDSIA nano-drone set and the UIUC propeller database. [The reference-data research](research/reference-flight-data.md) lists each source with its licence.
- **Data under any other licence,** such as CC BY-NC-SA, or GPL data such as the chirp logs.

## Data we use but don't commit

Tests that need uncommitted data download it into a local cache when it's needed, outside the repo's tracked files, and skip with a note when it's missing. Such a Scenario never blocks a merge until its data's licence is cleared. A nightly run reports how those Scenarios do, without blocking anything.

The good Blackbox log decoders are GPL or AGPL, so they're only ever run as separate programs, never built into OpenDrone, and a Blackbox log a test reads back is converted to CSV first.

## How credit is kept

- [`CREDITS.md`](../CREDITS.md) lists every outside asset and dataset in the repo with its source, its licence and a checksum. It's generated from the texture manifest, and lists the outside sound files the same way.
- CC BY data carries its attribution notice beside its files, as well as in `CREDITS.md`.
- A Pack names its licence in its manifest. The built-in Pack is CC0 throughout. A Pack that mixes licences lists the exceptions by path, with a credit line for anything under CC BY ([#16 §12](https://github.com/BartoszSolkaBD/OpenDrone/issues/16)).
- Research notes cite every source, with its licence.
