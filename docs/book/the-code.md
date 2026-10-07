# The code (rustdoc)

The reference for OpenDrone's code, made by rustdoc from the comments in the code each time the book is built. It includes the crates' internal items, not only what one crate offers another, so it documents how each crate works inside.

OpenDrone is one Cargo workspace of twelve crates ([ADR-0003](../adr/0003-crate-split-and-flight-inputs.md)). The five **core** crates make up the Simulation: they're bit-exact on every computer and follow the house rules ([ADR-0001](../adr/0001-bit-exact-determinism-with-ordinary-floats.md)). The **edge** crates read files and devices and play sound around it, **the game** is the only crate that uses Bevy, and `xtask` holds the developer tools, which are never shipped. Who may use whom is checked in CI by `cargo xtask walls`.

{{#crate-list ../../crates}}
