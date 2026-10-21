# Credits

This file lists every outside asset and dataset in OpenDrone, each with its source, its licence and a checksum ([ADR-0014](docs/adr/0014-licences-for-libraries-and-assets.md)). **It is generated** from the texture manifest, [`assets-src/materials.toml`](assets-src/materials.toml), by `cargo xtask credits`; don't edit it by hand. CI checks it is up to date. Only CC0 assets and CC BY data enter the repo.

## Textures

Poly Haven and ambientCG pictures, all CC0 1.0: anyone may use them for any purpose, with no credit needed, and they are credited here all the same. They are not in the repo: `cargo xtask textures` downloads each into `assets-src/textures/`, and refuses a file whose SHA-256 differs from the one below. A Map's `.glb` carries only a material's name; the game swaps the pictures in by name.

| Texture | Source | By | Licence | File | SHA-256 |
|---|---|---|---|---|---|
| [Concrete042A](https://ambientcg.com/a/Concrete042A) | ambientCG | ambientCG | CC0-1.0 | diffuse | `22986233ff2b78703bc6808a351cafea0da80bfb76a932675034722a356f691c` |
|  |  |  |  | normal | `404bc2653f47bfde272792048091431998baa879599cc682b01903eecd18414f` |
| [Grass001](https://ambientcg.com/a/Grass001) | ambientCG | ambientCG | CC0-1.0 | diffuse | `b9b6d61bc3b6137b868a3447eef18737896acd26d58fe2f4b83ce8c0e9d3f8ad` |
|  |  |  |  | normal | `eef0b56db5f00a6fcb3d0e0f8463e4e141e88aa85b26527a04d617b75a4ab5d2` |
| [asphalt_04](https://polyhaven.com/a/asphalt_04) | Poly Haven | Jenelle van Heerden, Sergej Majboroda | CC0-1.0 | diffuse | `837a78bb1e94864c221f847c85480484e953a9ee958772481dd2c501e18def2e` |
|  |  |  |  | normal | `18b91c2a6d83a8fbaa8d7fe84e80a66bd6451e4ec2774a09bf4429498c8f3912` |
| [bark_brown_02](https://polyhaven.com/a/bark_brown_02) | Poly Haven | Rob Tuytel | CC0-1.0 | diffuse | `920fa0bed0c9d78c1d530e99795113afd532d7db746de440f4a85d7c83ed0f1a` |
|  |  |  |  | normal | `0d5e691e8ad8bcd093a3587887fda2a6b4a948b5708a4f256f7919584c6eb857` |
| [concrete_floor_worn_001](https://polyhaven.com/a/concrete_floor_worn_001) | Poly Haven | Dimitrios Savva, Rico Cilliers | CC0-1.0 | diffuse | `6e40c0fc908f4d66431836f5abe203f3a4eb064e88824378a91a46a26e2f464c` |
|  |  |  |  | normal | `523bd94a1be5dd2f11c2ae2b7d7c77445cf5bd9e41604db515a7c8364dcba067` |
| [concrete_wall_008](https://polyhaven.com/a/concrete_wall_008) | Poly Haven | Dario Barresi, Charlotte Baglioni | CC0-1.0 | diffuse | `a203166d280d3cfc6a91bbc9ffdae406eb9555b4a4f658cd92619063eb17c147` |
|  |  |  |  | normal | `83cd3fa2c7cbf4c81402386065daf8fa6f72e1f1f77e02ccb1eedb28496909ae` |
| [corrugated_iron](https://polyhaven.com/a/corrugated_iron) | Poly Haven | Jenelle van Heerden, Dimitrios Savva | CC0-1.0 | diffuse | `7f3649dd0d056a55a910146f09b1d5298370696a90f511a255fb4c5715bb4b58` |
|  |  |  |  | normal | `e98b839ae681ba929eb80f07348c70d0731b48239abe69268effab18420cd271` |
| [metal_plate](https://polyhaven.com/a/metal_plate) | Poly Haven | Rob Tuytel | CC0-1.0 | diffuse | `c6b6739eac2c75ccc898e753d7833df24eafbdb9aa92fb6eabfb92abd1a680a5` |
|  |  |  |  | normal | `994f851a84d52b51e97906021a1b7070ee6eac9a789fef041132268de47d8e6b` |
| [plastered_wall_04](https://polyhaven.com/a/plastered_wall_04) | Poly Haven | Rob Tuytel | CC0-1.0 | diffuse | `52d789111bbe93d311c22e49d28b56256593c8f84c75742642403da857a36503` |
|  |  |  |  | normal | `c0a41dc06e33072ec1156d89f3bd4b481a479616791d4e9df6c5ee7b3283db82` |
| [plywood](https://polyhaven.com/a/plywood) | Poly Haven | Rob Tuytel | CC0-1.0 | diffuse | `800905650fc6cab2fe7c77c41851f8ea89669f1287ad5ce67c48f435654c0af8` |
|  |  |  |  | normal | `a0d8459eb2586435790225440319586775c9b7dff8441062e077501fd0811e53` |
| [rusty_metal_02](https://polyhaven.com/a/rusty_metal_02) | Poly Haven | Rob Tuytel | CC0-1.0 | diffuse | `adaf094391518b6afe787a123942110aa88ef521c844f970b124420f5ed5d3d9` |
|  |  |  |  | normal | `995197a62cac6fbb000e6e6591a3330a3b77ef069c4ac58f505b586b9bdbd97b` |

## Not yet

- Sound files: the CC0 hit and menu clips, and each Map's Background Sound, are listed here the same way once they are in the repo.
- Outside fonts (OFL-1.1), if any are used. The OSD fonts are OpenDrone's own, under CC0.
- Reference datasets under CC BY.

Nothing else from outside is in the repo.

The Rust libraries OpenDrone uses aren't listed here: their licences are checked by cargo-deny against [`deny.toml`](deny.toml).
