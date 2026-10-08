# Changing a file format

Every Scenario and every Pack file starts with `format = N`. When a change adds an item those files must hold, or changes how a Pack file is written, the files move to the next format, and **one command rewrites them all**: `cargo xtask migrate <step>`. This page says when that's needed, how to do it, and what the tool does.

## Two format numbers

Scenarios and Pack files are numbered apart, so a new starting-state item never touches a Pack, and a Pack change never touches a Scenario.

| Files | Format today | Its steps live in |
|---|---|---|
| Every Scenario in `scenarios/`, and every Scenario the checks keep as a fixture under `crates/` | 1 | `SCENARIO_STEPS`, in [`crates/scenario/src/format.rs`](../crates/scenario/src/format.rs) |
| Every Pack file (`pack.toml`, `quad.toml`, `map.toml`, Input Device profiles), every Test Quad in `scenarios/test-quads/` and every built-in Test Map in [`crates/pack/test-maps/`](../crates/pack/test-maps/empty-air.toml) | 1 | `PACK_STEPS`, in [`crates/pack/src/migration.rs`](../crates/pack/src/migration.rs) |

The built-in Test Maps, such as `test/empty-air`, are built into the code, but each one's `map.toml` is a file of its own in `crates/pack/test-maps/`, so the tool rewrites them like any Pack file. A new Test Map goes there too, never as text inside the Rust code, and a readable check makes sure of it.

Scenarios are never upgraded in memory, so a Scenario step rewrites the checks' fixture Scenarios too, found by their `[start]` table. The Pack checker's fixture Packs in `crates/pack/tests/fixtures/` are left as they are: the Pack reader upgrades them in memory, which the checks then exercise.

Results files are never migrated: the Scenario runner writes them, with a `format` line of their own. `tune.txt` stays Betaflight CLI text with no `format` line ([ADR-0015](adr/0015-tune-is-betaflight-cli-text-spelling-out-every-setting.md)), and a Feel Test log is Markdown.

`cargo xtask migrate` on its own prints both numbers and lists every step.

## When a change needs a step

- **A new starting-state item,** such as a new Assist, several Quads in one Scenario, or a new switch. A Scenario spells out everything, every time, with no hidden defaults ([ADR-0002](adr/0002-scenario-starting-state-spells-out-everything.md)), so every Scenario must get the item written in, and the reader must refuse a Scenario without it.
- **A new or renamed item in a Pack file,** such as a new number every Quad definition must carry, or a sound-block key with a better name. The readers refuse unknown and missing keys, so any such change is a new format ([ADR-0011](adr/0011-packs-are-data-only-toml-named-pack-item.md)).

A change that only edits values, such as moving an Estimate in a Feel Test, needs no step.

**The step writes the value that keeps today's behaviour.** A new Assist is written off, a new number takes the value the code used before it was a number, a renamed key keeps its value. So a step never moves a Result. If the new item should change a flight, that's a separate change in the same pull request, after the step, whose moved Results are explained like any other.

## How to do it

In the pull request that brings the item:

1. **Teach the reader the item.** For a Scenario, `crates/scenario/src/read.rs`; for a Pack file, its reader in `crates/pack/src/` (for a Quad definition, `schema.rs`).
2. **Write the step** at the end of `SCENARIO_STEPS` or `PACK_STEPS`. A step has a name (lowercase words joined by dashes), the format it upgrades (the newest today), a plain sentence saying what it adds and why that keeps today's behaviour, and the change itself. Two helpers write an item lined up with its neighbours: `add_after` puts `key = value` on the line after another key of a table, and `add_to_inline_table` adds it to the end of an inline table such as `assists = { … }`. For example, if Auto-arm were a new Assist:

   ```rust
   pub const SCENARIO_STEPS: &[Step] = &[Step {
       name: "add-auto-arm",
       family: Family::Scenarios,
       from: 1,
       says: "Scenarios gain the Auto-arm Assist, off: before it existed, no Scenario armed by itself",
       rewrite: add_auto_arm,
   }];

   fn add_auto_arm(_: FileKind, doc: &mut DocumentMut) -> Result<(), String> {
       add_to_inline_table(doc, "start", "assists", "auto_arm", "off")
   }
   ```

   A Pack step is told which kind of file it's rewriting, so it can add a Quad definition's item to `quad.toml` and leave `pack.toml`, the Maps and the Test Quads alone, apart from their `format` line. The newest format follows the steps by itself: format 1 plus one per step.
3. **Run it:** `cargo xtask migrate add-auto-arm`. It lists every file it rewrote.
4. **Check nothing moved:** `cargo scenarios check` must find every Results file up to date, and after a Pack step `cargo xtask packs` must pass. Then run the rest of the checks in [Setup and CI](book/contributing.md).
5. **Update the docs that show the format,** such as the starting-state table in [Reading a Scenario](verification/reading-a-scenario.md) or a section in [Checking a Pack](verification/checking-a-pack.md).
6. **Say so in the pull request:** the step's name, and that no Result moved.

## What the tool does

- It rewrites each file of the step's family that is in the format the step upgrades: it writes the step's item and bumps the `format` line by one.
- It keeps every comment, blank line and space as it was. A new item is lined up with the item before it, with the same indent and its `=` in the same column. A comment above the next item stays with that item, and a comment beside the last item of an inline table stays beside it.
- A file already in the newer format is left alone, so running a step twice changes nothing.
- If any file can't take the step, such as one that needs an earlier step first, it lists every such file and writes nothing.
- It writes every file or none: each new text goes first to a hidden file beside its file (`.<name>.migrating`), and only when all of them are written do they replace the files. Only a failure while replacing them, which is rare, could leave some files rewritten; the tool then names each one.

## Older files

- **A pilot's older Pack** is upgraded in memory every time it's read, one format at a time, by the very same steps: every Pack reader does this, for manifests, Quad definitions, Test Quads and Maps (the built-in Test Maps too). The pilot's files on disk aren't changed. A Pack in a newer format than the game knows is refused: it "needs a newer OpenDrone" ([Checking a Pack](verification/checking-a-pack.md#file-formats)).
- **An older Scenario** is refused, naming the step that brings it up to date, for example on a branch started before the step arrived. Scenarios are never upgraded in memory, because a Scenario's file must show everything it depends on.
- **The Feel Test log rules** read the version before a change through the same upgrade, so a Pack step's new item doesn't count as a moved number.

## How it's proved

No real step exists yet. The readable checks use synthetic ones: each pretends an item the files hold today, such as the Auto-arm Assist or the Quads' `restart_tries`, arrived in a step from the format before the newest (a pretend format 0 while format 1 is the first). On a scratch copy of the repo, the step must give back every committed Scenario and Pack file byte for byte, so the item is there with today's value, every comment and line is in place and `format` is the newest; and every Scenario must still match its committed Results. The Pack reader's in-memory upgrade must give the same text as the tool, and read as the same Quad with the same fingerprint, and the Map reader must read an older `map.toml` through the same upgrade. The checks are in `crates/pack/tests/migration.rs` and `crates/scenario/tests/migration.rs`. One more checks that every Scenario and Pack file in the repo, the Test Maps' included, is in the newest format, so a step that was written but never run fails CI.
