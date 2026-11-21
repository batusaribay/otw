# over-the-wire

A Rust CLI for OverTheWire games

![over-the-wire in a terminal](preview.png)

## Why it exists

The OverTheWire pages are a website: game lists, level tasks, and rules are
spread across HTML pages, with source files hidden behind a JavaScript call.
This tool puts the whole path in the terminal, offline when needed, and in
plain text that pipes and searches.

## How it works

1. `overthewire.org/games.json` provides the game list, level counts, SSH hosts,
   and ports, including games with special level numbering.
2. Game, level, and rules pages are downloaded on demand, parsed as HTML, and
   rendered as width-aware plain text. Headings, lists, and code blocks stay
   readable without decorative markers.
3. Some pages load their source with a `renderCode("...")` call. Those files are
   fetched too and shown inline, so Vortex and Semtex source is present.
4. Pages are cached under `$XDG_CACHE_HOME/over-the-wire`, normally
   `~/.cache/over-the-wire`. Level pages last a week and the manifest six hours.
   Stale copies are used when the network is unavailable.
5. `search` walks every level page, matches case-insensitively, and prints a
   snippet around the first hit in each level.
6. Output is plain text. Every command that makes sense as data has a `--json`
   form.

## Usage

```bash
over-the-wire [OPTIONS] [COMMAND]

Commands:
  games   List every game and its server
  info    Show a game's overview and beginner notes
  levels  List the levels of a game
  level   Show the task for the level you are logged in as
  ssh     Print or run the ssh command for a level
  search  Search the notes of every level
  start   Show the suggested order for playing the games
  rules   Show the game rules
  page    Render any page from overthewire.org
  cache   Inspect or manage the local page cache

Options:
      --offline       Use only locally cached pages
      --refresh       Re-download pages instead of using the cache
      --links         Show link URLs after link text
  -w, --width <COLS>  Wrap output at COLS columns (0 disables wrapping)
  -h, --help          Print help
  -V, --version       Print version

Examples:
  over-the-wire games
  over-the-wire level bandit 3
  over-the-wire ssh bandit 3 --run
  over-the-wire search cron --game bandit
  over-the-wire --offline level bandit 30
  over-the-wire cache fetch --all
```

## Notes

- Game names are case-insensitive. Levels accept `4`, `bandit4`, `level4`, or
  `l4`. `ssh` with no level defaults to level 0. Natas is web based, so
  `over-the-wire ssh natas 3` prints the level URL instead of an SSH command.
- `level` takes the level you are logged in as. `over-the-wire level bandit 3`
  shows the task for bandit3, which is page 4. `level bandit` is shorthand for
  `level bandit 0`, the login page and first task. Krypton, Vortex, and Natas
  use the same number for both.
- `search` exits `1` when nothing matches, like `grep`. Other errors also exit
  `1`.
- The cache is safe to delete. `cache status` shows its location, size, and age.
- The game notes, goals, and source files belong to OverTheWire and remain with
  their authors. The tool fetches them at runtime and does not redistribute
  them.

## Install

**Requirements:** Rust 1.88+ and `git`.

```bash
cargo install --git https://github.com/batusaribay/over-the-wire.git --locked
```

## License

This project is licensed under the [MIT License](LICENSE).
