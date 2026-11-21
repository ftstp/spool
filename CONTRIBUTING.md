# Contributing to Spool

Thanks for wanting to make Spool better. Fixes, translations and features are welcome as
pull requests; the maintainer reviews each one and decides whether it goes in.

## How

1. Fork the repository and create a branch from `main`.
2. Make your change, with a test when it changes behaviour.
3. Check it: `cargo test --workspace` and `cd web && npx svelte-check --threshold error`.
   Commit with `git commit -s` (see "Licence and sign-off" below).
4. Open a pull request saying what it changes and why. Screenshots help for anything visible.

For anything big (a new feature, a change of how something works), open an issue first so we
can agree on the approach before you spend time on it.

## The rules that are not negotiable

These protect people's work. A pull request that breaks one will not be merged, however good
it is otherwise.

### 1. Never modify or delete a user's file

Spool reads projects; a repair or a treatment writes a NEW file next to the original and never
overwrites anything (`repair::safe`: create-only writes). Files move on disk only when the
user turned on physical arranging, every move is journaled and undoable, and a move that fails
halfway puts everything back. Nothing is ever deleted: what is removed goes to a `.corbeille`
folder (Spool's trash folder; the name is French and kept as is).

### 2. Every version opens every older base

A user's base is years of sorting, notes and choices. Any later version of Spool must open a
base written by any earlier one, and keep everything in it.

- **Migrations are append-only** (`crates/store/src/lib.rs`, `MIGRATIONS`). Never edit or
  remove a shipped one: add the next. `shipped_migrations_are_frozen` fails otherwise.
- **Only additive changes**: new tables, new columns with a default, new settings keys. Never
  rename or drop a column or a table, never change what an existing column or settings key
  means, never reuse a settings key for something else.
- **JSON stored in the base** (settings, `data`, `actions`...): new fields are optional and
  read with a default; old fields stay readable.
- **A base from a newer version is refused**, not opened (it may hold what this version does
  not understand), and a base is copied to `backups/` before any migration runs.
- `a_base_from_0_1_0_opens_with_everything_in_it` opens a base written by 0.1.0
  (`crates/store/fixtures/`). At each release a fixture of that version is added the same way.

### 3. Every text in every language

No text in a component or in Rust code: every string the user reads is a key in
`web/src/lib/i18n/locales/<lang>/<namespace>.json`, in French AND English (`t()` in the
interface, `i18n::tr!` in Rust, namespace `core`). `cargo test -p i18n` checks that both
languages have the same keys. Never use the em-dash character (U+2014) in a text: use a hyphen.

A new language is a new folder of locale files: translations are very welcome.

### 4. Nothing leaves the user's computer

Spool sends nothing about the user or their projects anywhere. Its only network requests are the
update check and, if the user turns on the audio analysis, the one-time download of its model
(`crates/core/src/sound.rs`: fixed URLs, every file checked against a pinned SHA-256). A change
that adds any other network request needs to be discussed first.

## Licence and sign-off

Spool is free software under the GPL-3.0-or-later, and will stay free: it is not sold and
there is no paid edition. Ports to other DAWs and to macOS are very welcome.

Each commit of a pull request carries a sign-off, the
[Developer Certificate of Origin](https://developercertificate.org/): a line at the end of
the commit message saying you wrote the change (or have the right to submit it) and that it
may be published under the project's licence. `git commit -s` adds it:

```
Signed-off-by: Your Name <your@email>
```

You keep the copyright of what you write; it is published under the GPL-3.0-or-later like
the rest. No other agreement is asked.
