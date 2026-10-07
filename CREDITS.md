# Credits

This file will list every outside asset and dataset in OpenDrone, each with its source, its licence and a checksum ([ADR-0014](docs/adr/0014-licences-for-libraries-and-assets.md)):

- textures, from CC0 sources such as Poly Haven and ambientCG;
- sound files: the CC0 hit and menu clips, and each Map's Background Sound;
- outside fonts (OFL-1.1), if any are used (the OSD fonts are OpenDrone's own, under CC0);
- reference datasets under CC BY.

It will be generated from the texture manifest once the asset pipeline exists. Only CC0 assets and CC BY data enter the repo.

Nothing from outside is in the repo yet.

The Rust libraries OpenDrone uses aren't listed here: their licences are checked by cargo-deny against [`deny.toml`](deny.toml).
