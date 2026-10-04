# Can the simulation be deterministic across platforms?

Research for [#7](https://github.com/BartoszSolkaBD/OpenDrone/issues/7), part of the [alpha spec map](https://github.com/BartoszSolkaBD/OpenDrone/issues/1). It feeds [#11](https://github.com/BartoszSolkaBD/OpenDrone/issues/11) (determinism policy and Scenario format).

Checked on 2026-10-03 against Rust 1.99.0 (stable), LLVM `main`, glam 0.33, Bevy 0.19.1, Rapier 0.36, Avian 0.7, GGRS 0.13 and lightyear 0.30. Every claim cites a primary source (see [Sources](#sources)). Claims marked **Inference** are reasoning from those sources. Claims marked **Unconfirmed** found no primary source.

## Answer in brief

**Yes.** Ordinary hardware floating point (`f32`/`f64`) can give bit-identical Quad physics on macOS ARM64, Windows x86-64 and Linux x86-64. The Quad physics crate must follow a short list of rules, and the runtime cost is small.

The four basic operations and square root already give the same bits on every machine. Rust and LLVM both guarantee this. Every difference we found comes from a short list of known sources, and each one can be fixed:

1. **Platform maths functions** (`sin`, `cos`, `exp`, `powf`, `atan2`, …) differ by OS, by C library version, and even by CPU model. Fix: use the pure-Rust `libm` crate for all of them.
2. **SIMD vector maths** (glam's `Vec3A`, `Quat`, `Mat4`) runs different instruction sequences on ARM and x86. Fix: use scalar (non-SIMD) vector types in the physics crate.
3. **Order of work**: `HashMap` iteration, parallel threads, Bevy system order, and frame-time-based steps. Fix: a fixed tick, ordered collections, and a fixed order of updates.
4. **Edge cases**: NaN sign bits, and which zero `min`/`max` returns when given +0.0 and −0.0. Fix: treat a NaN as a Scenario failure, and normalise these values before taking a state fingerprint.

Soft-float and fixed-point maths would also give this guarantee, but only at a much higher cost. Neither buys anything extra on our targets or on future mobile targets. The most expensive mistake would be to settle for tolerance-only comparison. It closes the door on rollback multiplayer and on input-based ghost replays, and it makes long Scenarios with crashes fragile.

## How floating point behaves on our three platforms

### Already identical everywhere

- **Basic operations are exact IEEE 754.** This covers `+ - * / %`, `sqrt`, `mul_add`, `abs`, `as` casts and comparisons. Rust's float semantics say these "produce results that exactly match IEEE 754-2008 (with roundTiesToEven […] without abruptUnderflow/flush-to-zero)" ([RFC 3514][rfc3514]).
  - LLVM promises that such an instruction "returning a non-NaN value is guaranteed to always return the same bit-identical result on all machines and optimization levels" ([LLVM LangRef, Floating-Point Semantics][langref]).
- **All our targets comply.** "The most widely used targets (64bit x86 and 64bit ARM, and also the increasingly popular RISC-V) are fully compliant" ([RFC 3514][rfc3514]). The exceptions are 32-bit x86 (x87) and some old MIPS and 32-bit ARM targets, which we don't ship. The likely future targets, iOS and Android, are ARM64 and x86-64, so they comply too.
- **No automatic fused multiply-add (FMA).** An FMA computes `a*b + c` with one rounding instead of two, so it can change results. Rust never fuses on its own: "Providing strict IEEE 754-2008 guarantees precludes […] turning `a*b + c` into FMA operations" ([RFC 3514][rfc3514]). rustc emits plain LLVM `fadd`/`fmul` with no fast-math flags ([rustc builder.rs][rustc-builder]).
- **Compiler flags and auto-vectorisation don't change results.** LLVM vectorises floating-point reductions "only when at least the -fassociative-math […] subset of -ffast-math is used" ([LLVM Vectorizers][vectorizers]), and Rust never grants that.
  - So `-C target-cpu=native`, `+fma`, debug builds and release builds all give the same bits for plain arithmetic. **Inference** from the LangRef guarantee above.
  - `mul_add` is single-rounded whether it runs on hardware or in software. It is "guaranteed to be the rounded infinite-precision result" ([std `f64::mul_add`][std-muladd]).
- **The default floating-point environment.** Every platform starts with round-to-nearest and no flush-to-zero:
  - Linux: the x86-64 System V ABI ([psABI][psabi]).
  - Windows: the x64 calling convention ([Microsoft, MXCSR][ms-mxcsr]).
  - macOS: XNU's `FPCR_DEFAULT (0)` ([XNU proc_reg.h][xnu-fpcr]).
  - Rust assumes this environment, and changing it from inline assembly is undefined behaviour ([RFC 3514][rfc3514]).

### What differs, and why

| Source of difference | Evidence | Fix |
|---|---|---|
| **std maths functions.** These are `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `exp`, `exp2`, `exp_m1`, `ln`, `log*`, `ln_1p`, `powf`, `powi`, `cbrt`, `hypot`, the hyperbolics, `to_degrees` and `to_radians`. | std docs, on each one: "The precision of this function is non-deterministic. This means it varies by platform, Rust version, and can even differ within the same execution" ([std `f64::sin`][std-sin]). They call the platform C library: glibc, Microsoft UCRT or Apple libm ([std cmath.rs][std-cmath]). | Route every one through the `libm` crate. `sqrt`, `mul_add`, `abs`, `floor`, `ceil`, `round`, `trunc` and `div_euclid` are exact and safe. |
| **The platform C library picks code by CPU.** | Linux glibc chooses an FMA or plain variant of `sin`, `exp`, `pow`, `atan` and others at start-up from the CPU's features ([glibc ifunc-avx-fma4.h][glibc-ifunc]). Windows: "the CRT startup code detects whether the CPU supports FMA3 … Differences may also be observable between computers that do or don't support FMA3" ([Microsoft `_set_FMA3_enable`][ms-fma3]). | Same fix as above. Note: this breaks even **same-OS** bit-exactness, because GitHub's x86-64 runners don't promise a CPU model ([GitHub runners][gh-runners]). |
| **The platform C library changes between versions.** | glibc 2.41, 2.43 and 2.44 swapped in correctly rounded versions of several functions ([glibc NEWS][glibc-news]). glibc "does not aim for correctly rounded results" in general ([glibc manual][glibc-errors]). | Same fix as above. |
| **Compile-time constant folding.** | LLVM folds maths calls on constants with the *build machine's* C library: "we use the host native double versions … FIXME: Stop using the host math library" ([ConstantFolding.cpp][llvm-constfold]). | Same fix as above. `libm` is plain Rust, so folding its code is exact. |
| **SIMD vector maths (glam).** | glam's ARM (NEON) code "has always used fused multiply-add unconditionally". Since September 2026, its x86 (SSE2) code also uses FMA, but only "whenever the target supports it" ([glam PR #853][glam-853]). The maintainer: "for cross platform determinism you will probably also need to build with `scalar-math` enabled" ([glam discussion #388][glam-388]). Parry had to turn on `scalar-math` to fix its cross-platform determinism, because SIMD "caused floating-point non-associativity in Vec3/Vec4/Quat dot products" ([parry CHANGELOG 0.29.0][parry-changelog]). | Use scalar vector and quaternion types in the physics crate. See [Surprises](#surprises-that-affect-other-tickets) for why glam's `scalar-math` switch is awkward next to Bevy. |
| **Fast-maths opt-ins.** | `algebraic_add` and its siblings have been stable since Rust 1.98: "Algebraic operations are non-deterministic" ([std algebraic operators][std-algebraic]). `*_fast` intrinsics and `mul_add_relaxed` behave the same way. | Ban them in the physics crate. |
| **NaN bits.** | A NaN result "has a non-deterministic sign" ([std NaN bit patterns][std-nan]). x86 produces a negative NaN and ARM a positive one ([RFC 3514][rfc3514]). | A NaN in Quad state is a Scenario failure. Normalise NaN before fingerprinting. |
| **`min`/`max` given +0.0 and −0.0.** | "either input may be returned non-deterministically" ([std `f32::max`][std-f32]). Rapier hit exactly this: "the signed zero a min/max tie stores differs between SSE/wasm and NEON" ([Rapier CI workflow][rapier-ci]). | Normalise −0.0 in fingerprints. Avoid `min`/`max` where the sign of zero matters. |
| **Order of work.** | `HashMap` "is randomly seeded" and iterates "in arbitrary order" ([std HashMap][std-hashmap]). Bevy: "Unless the order is explicitly specified, their relative order is nondeterministic" ([Bevy example][bevy-order]). | Use `BTreeMap`, `Vec` or a fixed-seed hasher. Run physics in an explicit order. Combine any parallel results in a fixed order. |
| **Variable time step.** | Bevy's `Time<Fixed>` exists so logic "(like physics) … should have consistent behavior, regardless of framerate" ([Bevy `Time<Fixed>`][bevy-fixed]). | Step in whole ticks of a constant `dt`. Never use frame time. |
| **C code, for example a future SITL Betaflight.** | Unlike Rust, C compilers fuse by default. Clang's default is `-ffp-contract=on` ([Clang manual][clang-fpcontract]). GCC's default is `-ffp-contract=fast` outside strict ISO mode ([GCC manual][gcc-fpcontract]). | Build with `-ffp-contract=off`, no `-ffast-math`, and no platform maths functions. Otherwise SITL sits outside the determinism guarantee. |

## Options, their costs, and what each enables

| Option | Guarantee | Runtime cost | Engineering cost | Enables |
|---|---|---|---|---|
| **A. Tolerance only** | None. Results drift by platform and by CPU. | None | Low at first. Tolerances must absorb platform drift, which grows over time (see below). | Short Scenarios. Ghost replays that record state. Server-authoritative multiplayer. |
| **B. Same-machine repeatability** | The same binary on the same machine repeats itself. | Small | Needs the order-of-work and fixed-tick rules. Platform maths still varies by CPU, so even this needs `libm`. **It is not a cheaper middle ground.** | Reproducing bugs locally. Golden fingerprints on one OS. |
| **C. Cross-platform bit-exact, hardware floats** *(recommended)* | Identical bits on every IEEE-compliant 64-bit target, for a pinned toolchain and pinned dependencies. | Basic arithmetic: zero, because the instructions are the same. Maths functions: `libm` speed against platform maths is **unconfirmed**; measure it in the tracked physics benchmark. Vector maths: scalar instead of SIMD; no published figure for glam (**unconfirmed**). Rapier's `enhanced-determinism` mode now "costs no measurable performance" ([Rapier CHANGELOG 0.35][rapier-changelog]). | The rules list below, two lints and one CI job. Pinned toolchain. Every toolchain or maths-crate bump is reviewed as a behaviour change. | Everything in A and B. Ghost replays that record inputs. Rollback and lockstep multiplayer, including cross-play between Mac, PC and later mobile. Exact CI agreement checks. |
| **D. Soft-float** | Identical bits even on non-IEEE hardware. | Every arithmetic operation becomes a software routine of integer instructions. No project publishes a figure (**unconfirmed**). **Inference:** about 10 to 100 times slower per operation. | High. `rustc_apfloat` is an "API … completely unstable" port of LLVM's APFloat with no `sqrt` or trig ([rustc_apfloat][apfloat]). The `softfloat` crate (2023) has `sqrt`/`sin`/`cos`. No 2025–2026 crate aimed at games was found. No maths or physics library works with these types. | The same as C. The extra guarantee only matters for targets we will never ship. |
| **E. Fixed-point** | Identical bits; integer arithmetic only. | Add and subtract cost about the same as hardware floats. Multiply and divide on wide types are slower. `sqrt` "uses an iterative method, with up to 64 iterations" ([fixed][fixed]). Trig needs CORDIC, a step-by-step method. | Very high. `fixed` provides "No trigonometric functions" ([fixed][fixed]). The `cordic` crate was last released in 2021, and its own tests accept up to 0.001 error on `sin`/`cos` ([cordic source][cordic]). simba's fixed-point support leaves `powf`, `ln` and `exp2` `unimplemented!()`, so they panic at runtime ([simba fixed_impl.rs][simba-fixed]). Rapier marks fixed-point determinism ❌ ([Rapier announcement][rapier-2020]). **Inference:** quantities spanning many orders of magnitude, such as rotor inertia and thrust coefficients, need separate scaling per quantity. | The same as C. |

**Why tolerance-only is weaker than it looks (inference).** A Quad is a closed-loop system: the Flight Controller reacts to every small error, and contacts with a Map act like switches. A one-bit difference in a maths function can turn a grazing touch into a miss. After that, the two platforms fly different paths, and no sensible tolerance covers both. Tolerance-only Scenarios would then have to stay short, avoid contact, or use loose expectations.

### Rules for a deterministic Quad physics crate (option C)

1. Advance in whole ticks of a constant `dt`. Count Scenario time in ticks.
2. Call every maths function through one module backed by the `libm` crate. `libm` is a pure-Rust port of musl and CORE-MATH, kept by the Rust project ([libm README][libm]).
   - Its hardware shortcuts, `sqrt` and `fma`, are exactly specified IEEE operations, so they give the same bits as its software path ([libm arch][libm-arch]).
   - Ban the std versions with Clippy's `disallowed-methods` ([Clippy configuration][clippy-config]).
3. Use scalar `Vec3`/`Quat`/matrix types for simulation state, never SIMD-backed ones.
4. Never use `algebraic_*`, `*_fast` or `mul_add_relaxed`.
5. Never iterate a `HashMap`/`HashSet` where order matters; ban them with `disallowed-types`. Merge parallel work in a fixed order.
6. Draw randomness (turbulence, noise) from a seeded generator stored in the simulation state, never from system entropy or time.
7. Record inputs per tick as exact values, for example quantised Radio channel values.
8. Pin the Rust toolchain (`rust-toolchain.toml`) and commit `Cargo.lock`.
9. Use one float width (`f32` or `f64`) consistently.

**Precedent.** Rapier guarantees "the exact same results" across OS and processor under `enhanced-determinism` ([Rapier determinism guide][rapier-det]). Its CI compares golden hashes on `ubuntu-latest`, `ubuntu-24.04-arm` and wasm32 ([Rapier CI workflow][rapier-ci]). Avian hashes a 2D simulation on Windows, macOS and Linux ([Avian determinism test][avian-test]).

## What ghost replays and future multiplayer need

| Feature | How it works | Determinism needed |
|---|---|---|
| Ghost replay, **state recording** | Store position and attitude about 60 times a second and interpolate on playback. | **None.** Survives physics changes, game versions and platforms. |
| Ghost replay, **input recording** | Store inputs per tick and re-simulate. | **C**, plus an exact match of game version, physics constants and Quad definition. Any physics change invalidates old ghosts. Bonus: a run can be checked by re-simulating it. |
| **Rollback** multiplayer (GGRS/GGPO style) | Peers exchange only inputs and re-simulate after late inputs arrive. | **C**, across every platform that can join. GGRS: "This must hold across all participating clients, including across different CPU architectures" ([GGRS setup][ggrs-setup]). GGPO: "The game simulation must be fully deterministic" ([GGPO guide][ggpo]). Desyncs are caught by comparing checksums ([GGRS sessions][ggrs-sessions]). |
| **Lockstep** or input-only replication | Like rollback, without prediction. | **C**. lightyear: "The simulation needs to be deterministic" ([lightyear README][lightyear]). |
| **Server-authoritative** with client prediction | The server simulates. Clients predict and snap back when the server disagrees ([lightyear prediction][lightyear-pred]). | **Not required.** Determinism only reduces corrections. An explicit "not required" statement is **unconfirmed**. |

**File size (arithmetic).** A state sample of position (3 × `f32`) plus attitude (4 × `f32`) is 28 bytes. At 60 Hz that is about 1.7 kB/s. Inputs of about 8 bytes per tick at a 1 kHz physics rate come to about 8 kB/s before compression. With a high physics rate, input-recorded ghosts are not automatically smaller. Recommendation: make state-recorded ghosts the default, and treat input-recorded ghosts as a bonus that option C allows.

**A conflicting source.** bevy_ggrs advises that floats are "deterministic on the same platform, but not across different CPUs or operating systems … consider fixed-point math" ([bevy_ggrs desync guide][bevy-ggrs]). GGRS lists the same worry ([GGRS setup][ggrs-setup]). That is the cautious default when nobody controls maths functions and SIMD. Rapier's guarantee and cross-OS CI, together with the LLVM and Rust guarantees above, show that controlling those two sources is enough.

## Keeping Scenarios stable in CI on all three runners

- **Tolerances stay the readable contract.** The [verification rules](../context/verification.md) say expectations are plain values with tolerances. Under option C, tolerances describe what physics is acceptable. They no longer have to absorb platform noise.
- **Add two machine checks**, so no golden hashes are committed and reviewers see no noise:
  1. *Repeat check:* each runner runs every Scenario twice and compares a fingerprint of the final state, meaning a hash of the normalised state bits.
  2. *Cross-platform agreement:* the macOS, Windows and Linux jobs each upload their fingerprints, and a final job fails if any Scenario disagrees. It names the Scenario and the first tick where the platforms diverge.
- **Keep an ARM job.** Rapier's signed-zero bug "diverged serialized snapshots on aarch64 for months without any x86-only job noticing" ([Rapier CI workflow][rapier-ci]). Our `macos-latest` runner is ARM64 (macOS 26), and `ubuntu-latest` and `windows-latest` are x86-64 ([GitHub runners][gh-runners]), so both families are covered.
- **Bumps are behaviour changes.** Updating the Rust toolchain, `libm`, or the maths and physics crates can move fingerprints. That PR shows which Scenario expectations moved, as the verification rules require.

## Recommendation for #11

- **Policy:** cross-platform bit-exact (option C) for the Quad physics crate and the Flight Controller. Enforce it with the lints and the CI agreement job.
- **Scenarios:** keep human-readable expectations with tolerances. Determinism is a separate machine check, not something Scenario authors write.
- **Ghosts:** record state by default. Input recording becomes possible once option C holds.
- **Multiplayer:** option C keeps rollback and lockstep open. Server-authoritative play works either way.
- **Don't** adopt soft-float or fixed-point. They cost far more and add no guarantee our targets need.

## Surprises that affect other tickets

- **Rapier's determinism mode currently fails to compile with Bevy.** This affects #2, #12 and Map collisions. bevy_rapier's changelog (Unreleased, current master) says: "the `enhanced-determinism` feature currently fails to compile with bevy, because parry enables `glam/scalar-math` which removes the serde impls … that `bevy_reflect` requires" ([bevy_rapier CHANGELOG][bevy-rapier-changelog]).
  - The cause is Cargo's feature merging: "Cargo will use the union of all features enabled on that dependency" ([Cargo features][cargo-unification]). Any crate in the game binary that turns on glam's `scalar-math` turns it on for Bevy's rendering maths too.
  - **Inference:** the Quad physics crate should not use glam for simulation state. It should use its own small scalar maths types, or a maths crate that Bevy doesn't share.
- **Avian 0.7 is not proven cross-platform deterministic in 3D.** Its `enhanced-determinism` feature uses parry 0.27, which lacks the `scalar-math` fix from 0.29 ([parry CHANGELOG][parry-changelog]). Its cross-platform test is 2D only, and it runs on all three OSes only for labelled PRs ([Avian CI][avian-ci]).
- **SITL Betaflight (#5) would break determinism by default.** C compilers fuse multiply-adds by default (see the table above). A separately timed process also needs lockstep with the simulation. Treat a SITL Flight Controller as outside the determinism guarantee unless both are solved.
- **Physics rate (#10) and Settings (#13).** Any physics-side noise, such as turbulence or prop wash, must come from a seeded generator kept in the simulation state. Camera-only analog noise is not affected.
- **CI blueprint (#15).** It needs a pinned toolchain, the agreement job across three OSes, and the Clippy bans in the physics crate.

## Sources

[rfc3514]: https://github.com/rust-lang/rfcs/blob/fdb511cf68e5f9262994b49c2212466b096d3870/text/3514-float-semantics.md
[rustc-builder]: https://github.com/rust-lang/rust/blob/db8f076d2619ce2585b0380dda06e8da25a40da4/compiler/rustc_codegen_llvm/src/builder.rs#L522-L536
[langref]: https://llvm.org/docs/LangRef.html#floating-point-semantics
[vectorizers]: https://llvm.org/docs/Vectorizers.html#reductions
[llvm-constfold]: https://github.com/llvm/llvm-project/blob/7e48d8c0b5f9401798f09ec11a68215a4560d715/llvm/lib/Analysis/ConstantFolding.cpp#L2906-L2908
[std-sin]: https://doc.rust-lang.org/stable/std/primitive.f64.html#method.sin
[std-muladd]: https://doc.rust-lang.org/stable/std/primitive.f64.html#method.mul_add
[std-f32]: https://doc.rust-lang.org/stable/std/primitive.f32.html#method.max
[std-nan]: https://doc.rust-lang.org/stable/std/primitive.f32.html#nan-bit-patterns
[std-algebraic]: https://doc.rust-lang.org/stable/std/primitive.f32.html#algebraic-operators
[std-cmath]: https://github.com/rust-lang/rust/blob/db8f076d2619ce2585b0380dda06e8da25a40da4/library/std/src/sys/cmath.rs
[std-hashmap]: https://doc.rust-lang.org/std/collections/struct.HashMap.html
[cargo-unification]: https://doc.rust-lang.org/cargo/reference/features.html#feature-unification
[clippy-config]: https://doc.rust-lang.org/nightly/clippy/lint_configuration.html#disallowed-methods
[libm]: https://github.com/rust-lang/compiler-builtins/blob/7f5edc1fc0f7b736d9beb6f817650feac7247114/libm/README.md
[libm-arch]: https://github.com/rust-lang/compiler-builtins/blob/7f5edc1fc0f7b736d9beb6f817650feac7247114/libm/src/math/arch/mod.rs
[psabi]: https://gitlab.com/x86-psABIs/x86-64-ABI/-/blob/e1ce098331da5dbd66e1ffc74162380bcc213236/x86-64-ABI/low-level-sys-info.tex
[ms-mxcsr]: https://learn.microsoft.com/en-us/cpp/build/x64-calling-convention#mxcsr
[xnu-fpcr]: https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/osfmk/arm64/proc_reg.h#L838
[glibc-ifunc]: https://sourceware.org/git/?p=glibc.git;a=blob;f=sysdeps/x86_64/fpu/multiarch/ifunc-avx-fma4.h;hb=db6d1da22e65327ae29dcf9bc50986b2e8f3f0bd
[glibc-news]: https://sourceware.org/git/?p=glibc.git;a=blob;f=NEWS;hb=db6d1da22e65327ae29dcf9bc50986b2e8f3f0bd
[glibc-errors]: https://sourceware.org/glibc/manual/latest/html_node/Errors-in-Math-Functions.html
[ms-fma3]: https://learn.microsoft.com/en-us/cpp/c-runtime-library/reference/get-fma3-enable-set-fma3-enable
[clang-fpcontract]: https://clang.llvm.org/docs/UsersManual.html#cmdoption-ffp-contract
[gcc-fpcontract]: https://gcc.gnu.org/onlinedocs/gcc/Optimize-Options.html#index-ffp-contract
[gh-runners]: https://docs.github.com/en/actions/reference/runners/github-hosted-runners
[glam-853]: https://github.com/bitshifter/glam-rs/pull/853
[glam-388]: https://github.com/bitshifter/glam-rs/discussions/388
[parry-changelog]: https://github.com/dimforge/parry/blob/master/CHANGELOG.md
[rapier-det]: https://rapier.rs/docs/user_guides/rust/determinism
[rapier-changelog]: https://github.com/dimforge/rapier/blob/master/CHANGELOG.md
[rapier-ci]: https://github.com/dimforge/rapier/blob/master/.github/workflows/rapier-ci-build.yml
[rapier-2020]: https://dimforge.com/blog/2020/08/25/announcing-the-rapier-physics-engine
[bevy-rapier-changelog]: https://github.com/dimforge/rapier/blob/master/bindings/bevy_rapier/CHANGELOG.md
[avian-test]: https://github.com/avianphysics/avian/blob/main/src/tests/determinism_2d.rs
[avian-ci]: https://github.com/avianphysics/avian/blob/main/.github/workflows/rust.yaml
[fixed]: https://docs.rs/fixed/1.31.0/fixed/
[cordic]: https://docs.rs/crate/cordic/0.1.5/source/src/lib.rs
[simba-fixed]: https://github.com/dimforge/simba/blob/master/src/scalar/fixed_impl.rs
[apfloat]: https://docs.rs/rustc_apfloat/latest/rustc_apfloat/
[ggrs-setup]: https://github.com/gschup/ggrs/blob/main/docs/setup.md
[ggrs-sessions]: https://github.com/gschup/ggrs/blob/main/docs/sessions.md
[bevy-ggrs]: https://github.com/gschup/bevy_ggrs/blob/main/docs/debugging-desyncs.md
[ggpo]: https://github.com/pond3r/ggpo/blob/master/doc/DeveloperGuide.md
[lightyear]: https://github.com/cBournhonesque/lightyear/blob/main/README.md
[lightyear-pred]: https://github.com/cBournhonesque/lightyear/blob/main/book/src/concepts/advanced_replication/prediction.md
[bevy-fixed]: https://docs.rs/bevy/0.19.1/bevy/time/struct.Fixed.html
[bevy-order]: https://github.com/bevyengine/bevy/blob/v0.19.1/examples/ecs/nondeterministic_system_order.rs

- **Rust:**
  - [RFC 3514 float semantics][rfc3514]
  - std `f32`/`f64` docs: [sin][std-sin], [mul_add][std-muladd], [max][std-f32], [NaN bit patterns][std-nan], [algebraic operators][std-algebraic]
  - [std cmath.rs][std-cmath]
  - [rustc builder.rs][rustc-builder]
  - [std HashMap][std-hashmap]
  - [Cargo feature unification][cargo-unification]
  - [Clippy configuration][clippy-config]
- **The `libm` crate:** [README][libm], [arch module][libm-arch]
- **LLVM:** [LangRef][langref], [Vectorizers][vectorizers], [ConstantFolding.cpp][llvm-constfold]
- **Platform C libraries and ABIs:**
  - glibc: [ifunc selector][glibc-ifunc], [NEWS][glibc-news], [manual][glibc-errors]
  - Microsoft: [`_set_FMA3_enable`][ms-fma3], [x64 MXCSR][ms-mxcsr]
  - [x86-64 psABI][psabi]
  - [XNU FPCR][xnu-fpcr]
- **C compilers:** [Clang `-ffp-contract`][clang-fpcontract], [GCC `-ffp-contract`][gcc-fpcontract]
- **CI:** [GitHub-hosted runners][gh-runners]
- **Maths and physics crates:**
  - glam: [PR #853][glam-853], [discussion #388][glam-388]
  - [parry CHANGELOG][parry-changelog]
  - Rapier: [determinism guide][rapier-det], [CHANGELOG][rapier-changelog], [CI][rapier-ci], [2020 announcement][rapier-2020], [bevy_rapier CHANGELOG][bevy-rapier-changelog]
  - Avian: [determinism test][avian-test], [CI][avian-ci]
  - [fixed][fixed], [cordic][cordic], [simba fixed-point][simba-fixed], [rustc_apfloat][apfloat]
- **Netcode:**
  - GGRS: [setup][ggrs-setup], [sessions][ggrs-sessions]
  - [bevy_ggrs desync guide][bevy-ggrs]
  - [GGPO developer guide][ggpo]
  - lightyear: [README][lightyear], [prediction][lightyear-pred]
- **Bevy:** [`Time<Fixed>`][bevy-fixed], [system order example][bevy-order]
