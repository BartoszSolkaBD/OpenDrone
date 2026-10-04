# Libraries must be permissive or MPL-2.0, and agent-made assets are CC0

The code is MIT/Apache-2.0. Two licence rules sit around it, and both are hard to undo once a release has shipped:

- **Outside libraries** may use permissive licences only: MIT, Apache-2.0, BSD, ISC, Zlib, Unicode, CC0 and BSL-1.0, plus OFL for fonts. MPL-2.0 is also allowed. GPL, AGPL, LGPL and code with no licence are never linked. Libraries come from crates.io; one taken straight from git needs a written reason in the policy file. cargo-deny enforces all of this in CI.
- **Assets that agents make** are dedicated to the public domain under **CC0**: `.glb` Maps and Quad models, baked lighting and renders. The Blender scripts that make them are code, so they stay MIT/Apache-2.0, which Blender's GPL accepts for scripts.

Decided in [#15](https://github.com/BartoszSolkaBD/OpenDrone/issues/15), answering the question the [asset pipeline research](https://github.com/BartoszSolkaBD/OpenDrone/issues/8) left open.

## Considered options

- **Permissive libraries only.** Rejected. It blocks common helpers: the usual "find the settings folder" crate, `dirs`, pulls in `option-ext`, which is MPL-2.0. MPL's copyleft covers only that library's own files, and shipping it means pointing to its source, which is on crates.io anyway.
- **Also allowing LGPL.** Rejected. Rust builds one program from all its libraries, which makes LGPL's relinking duty awkward for a game.
- **Assets under MIT/Apache, like the code.** Rejected. Both licences are written for software. The US Copyright Office says purely machine-made output generally can't be copyrighted, so a licence claim on it may not hold.
- **Assets under CC BY 4.0.** Rejected for the same doubt, and because it would ask every reuser for a credit we may not be able to require.

## Consequences

- CC0 matches our texture sources, Poly Haven and ambientCG. Anyone may reuse an OpenDrone Map or Quad model anywhere, without credit.
- **`CREDITS.md`** lists every outside asset and dataset with its source, licence and checksum. It's generated from the texture manifest.
- **What enters the repo:** only CC0 assets and CC BY data, as [#11](https://github.com/BartoszSolkaBD/OpenDrone/issues/11) settled for data. Data with no licence is downloaded when needed and never committed. A one-page data policy in the docs says so.
- **GPL tools** such as `blackbox_decode` and SITL Betaflight run only as separate programs, never linked.
- **Security:** a library with a known security problem blocks the PR that adds or upgrades it. A daily scan opens an issue for problems found later, rather than blocking unrelated PRs.
