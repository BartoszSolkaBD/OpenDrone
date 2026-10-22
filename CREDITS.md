# Credits

This file lists every outside asset and dataset in OpenDrone, each with its source, its licence and a checksum ([ADR-0014](docs/adr/0014-licences-for-libraries-and-assets.md)). **It is generated** from the texture manifest, [`assets-src/materials.toml`](assets-src/materials.toml), and the list of datasets, [`assets-src/datasets.toml`](assets-src/datasets.toml), by `cargo xtask credits`; don't edit it by hand. CI checks it is up to date. Only CC0 assets and CC BY data enter the repo.

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
| [brown_mud_dry](https://polyhaven.com/a/brown_mud_dry) | Poly Haven | Rob Tuytel | CC0-1.0 | diffuse | `a56281b644de614787340f3db45e28d2f72ec32fc19a442f99002bae0ab11398` |
|  |  |  |  | normal | `4dd82ecb3e810d4de49d34c7d6cc30152129434a58b3bd786bc9952bc8874877` |
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
| [red_brick](https://polyhaven.com/a/red_brick) | Poly Haven | Rob Tuytel | CC0-1.0 | diffuse | `6dea3a2b5a3185ff48d299d182c405a9f787129d79677792db6de4afd81c47f6` |
|  |  |  |  | normal | `99bd2c60dfdb02b011cc3551bd9ab51d9b0be2807e3ba8f7a53c8a4c945b2f87` |
| [rusty_metal_02](https://polyhaven.com/a/rusty_metal_02) | Poly Haven | Rob Tuytel | CC0-1.0 | diffuse | `adaf094391518b6afe787a123942110aa88ef521c844f970b124420f5ed5d3d9` |
|  |  |  |  | normal | `995197a62cac6fbb000e6e6591a3330a3b77ef069c4ac58f505b586b9bdbd97b` |

## Datasets

Outside data under CC BY 4.0, committed only as trimmed CSV files, each folder with its attribution notice beside the files ([the data policy](docs/data-policy.md)). Anyone may use them for any purpose, crediting their makers as the notice says. Data with no licence, such as NeuroBEM, is downloaded when it's needed and never committed, so it isn't listed here.

| Dataset | By | Licence | File | SHA-256 |
|---|---|---|---|---|
| [Race Against the Machine](https://github.com/tii-racing/drone-racing-dataset) | TII Racing: Bosello, Aguiari, Keuter, Pallotta, Kiade, Caminati, Pinzarrone, Halepota, Panerati and Pau | CC-BY-4.0 | [`hovering-in-flight-01a-motors.csv`](scenarios/quads/freestyle-5/tii-logs/hovering-in-flight-01a-motors.csv) | `b34d20d345753723998ecbfc52eb0683e02c64023092d07b8add2519f43a27d9` |
|  |  |  | [`hovering-in-flight-01a-recorded.csv`](scenarios/quads/freestyle-5/tii-logs/hovering-in-flight-01a-recorded.csv) | `df6c20f681998577e566d5a052c34381361de9f1ee986f5c4f40017fea44b4d5` |
|  |  |  | [`hovering-in-flight-04a-motors.csv`](scenarios/quads/freestyle-5/tii-logs/hovering-in-flight-04a-motors.csv) | `e61024dcdcf3ec5340acd121b44d4ef157a17c93c1d4fc62e75c03a502e981ec` |
|  |  |  | [`hovering-in-flight-04a-recorded.csv`](scenarios/quads/freestyle-5/tii-logs/hovering-in-flight-04a-recorded.csv) | `a5ce822dd38d5f3f5af1b3026eac8a7ab6aa07420a010de7ac72241e43344c54` |
|  |  |  | [`punching-out-in-flight-01a-motors.csv`](scenarios/quads/freestyle-5/tii-logs/punching-out-in-flight-01a-motors.csv) | `c30299179f0a8b7a4dd89712f63041730e3007f2fb14fc988428fb52755d4a43` |
|  |  |  | [`punching-out-in-flight-01a-recorded.csv`](scenarios/quads/freestyle-5/tii-logs/punching-out-in-flight-01a-recorded.csv) | `a5c5536436fe38e8673e653b40278269d873b11e62658a4557f816bbde11528c` |
|  |  |  | [`punching-out-in-flight-04a-motors.csv`](scenarios/quads/freestyle-5/tii-logs/punching-out-in-flight-04a-motors.csv) | `8408994538e66bf4fbcdd7507d350243257209491750e8b308ebc2583e35475b` |
|  |  |  | [`punching-out-in-flight-04a-recorded.csv`](scenarios/quads/freestyle-5/tii-logs/punching-out-in-flight-04a-recorded.csv) | `2c8e6d8114eb992554eaa65cc3d9ab09fbef0fbb5983efe393792df5bdc57039` |
|  |  |  | [`slowing-from-3-m-s-in-flight-01a-motors.csv`](scenarios/quads/freestyle-5/tii-logs/slowing-from-3-m-s-in-flight-01a-motors.csv) | `1a7a48ce9b2cc78fbbd851676825842820cc6890dd511c047d253910492eb007` |
|  |  |  | [`slowing-from-3-m-s-in-flight-01a-recorded.csv`](scenarios/quads/freestyle-5/tii-logs/slowing-from-3-m-s-in-flight-01a-recorded.csv) | `3e00a5e54a12ec7633d2b7c43e2312a8e265f89e3861db40a32a66225be74590` |
|  |  |  | [`slowing-from-3-m-s-in-flight-04a-motors.csv`](scenarios/quads/freestyle-5/tii-logs/slowing-from-3-m-s-in-flight-04a-motors.csv) | `f4cc56dfc689564bddcb1351f76fe46a347488e22944f13bdb37bc1d1b03773e` |
|  |  |  | [`slowing-from-3-m-s-in-flight-04a-recorded.csv`](scenarios/quads/freestyle-5/tii-logs/slowing-from-3-m-s-in-flight-04a-recorded.csv) | `3b67ebef29ad1801399398bae1503494a103928e49fbfc947538fe649e414048` |
|  |  |  | [`slowing-from-5-m-s-in-flight-01a-motors.csv`](scenarios/quads/freestyle-5/tii-logs/slowing-from-5-m-s-in-flight-01a-motors.csv) | `a3fc14895a8729881664c221668cc1616e4ae2be6f9df20fed360dd4c2eafab4` |
|  |  |  | [`slowing-from-5-m-s-in-flight-01a-recorded.csv`](scenarios/quads/freestyle-5/tii-logs/slowing-from-5-m-s-in-flight-01a-recorded.csv) | `3795288edb844ce0f85554abb5257be435081a35b04a9b7f04b2ff83327fe896` |
|  |  |  | [`slowing-from-5-m-s-in-flight-04a-motors.csv`](scenarios/quads/freestyle-5/tii-logs/slowing-from-5-m-s-in-flight-04a-motors.csv) | `3f0c96e66871ae498e2a174bd9b74086a6ada2a9ba78a54721d4a27cc8d21edc` |
|  |  |  | [`slowing-from-5-m-s-in-flight-04a-recorded.csv`](scenarios/quads/freestyle-5/tii-logs/slowing-from-5-m-s-in-flight-04a-recorded.csv) | `812bfbdfd1460a806f727fa081a751661c52e847a8ed0ff5a13a731c4d88ce3b` |

## Not yet

- Sound files: the CC0 hit and menu clips, and each Map's Background Sound, are listed here the same way once they are in the repo.
- Outside fonts (OFL-1.1), if any are used. The OSD fonts are OpenDrone's own, under CC0.

Nothing else from outside is in the repo.

The Rust libraries OpenDrone uses aren't listed here: their licences are checked by cargo-deny against [`deny.toml`](deny.toml).
