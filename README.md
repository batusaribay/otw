# over-the-wire

The OverTheWire wargames in your terminal.

![over-the-wire in a terminal](preview.png)

## What it is

A single Rust binary that reads the [OverTheWire](https://overthewire.org/wargames/)
wargames as terminal text. It covers every game on the site: Bandit, Natas,
Leviathan, Krypton, Narnia, Behemoth, Utumno, Maze, Vortex, Manpage, Drifter,
FormulaOne, Semtex and the released conference games.

It is not a game client. It prints the same overviews, level goals, command hints,
reading lists and included source files that the website shows. It never asks for a
password, and it contains no walkthroughs.

## Why this exists

The OverTheWire pages are the manual. A level goal says what to look for, the
commands section points at the right man pages, and the included source explains the
binary. Those notes are essential for beginners, but they live in a browser and get
lost behind tabs.

Everything is better in a terminal. `over-the-wire` puts the notes in the same
scrollback as your SSH session, makes them greppable and scriptable, and keeps an
offline copy so a bad connection does not stop you from reading the level you are on.

## How it works

1. `overthewire.org/games.json` provides the game list, level counts, SSH hosts and
   ports, including the games that use special level numbering.
2. Game, level and rules pages are downloaded on demand, parsed as HTML, and rendered
   as width-aware plain text. Headings, lists, code blocks and inline commands
   survive the trip.
3. Some pages load their source with a `renderCode("...")` JavaScript call. Those
   files are fetched too and shown inline, so Vortex and Semtex source code is
   present in the terminal.
4. Pages are cached under `$XDG_CACHE_HOME/over-the-wire` (normally
   `~/.cache/over-the-wire`). Level pages are kept for a week and the game manifest
   for six hours; stale copies are used when the network is unavailable.
5. `search` walks every level page, matches case-insensitively, and prints a snippet
   around the first hit in each level.
6. Styling is monochrome: bold, dim, italic and underline only. Output is plain when
   piped, and every command that makes sense as data has a `--json` form.

## Usage

### Commands

| Command | Action |
| --- | --- |
| `over-the-wire` | Print help. |
| `over-the-wire games` | List games, level ranges and SSH servers. |
| `over-the-wire info <game>` | Show a game's overview and beginner notes. |
| `over-the-wire levels <game>` | List every level page of a game. |
| `over-the-wire level <game> <n>` | Show the goal, commands and reading for one level. |
| `over-the-wire ssh <game> [level]` | Print the SSH command for a level. |
| `over-the-wire ssh <game> [level] --run` | Connect with SSH. |
| `over-the-wire search <text>` | Search every level's notes. |
| `over-the-wire start` | Show the suggested order for playing the games. |
| `over-the-wire rules` | Show the wargame rules. |
| `over-the-wire page <path>` | Render any overthewire.org page. |
| `over-the-wire cache status` | Show cache location, size and age. |
| `over-the-wire cache fetch --all` | Download everything for offline reading. |
| `over-the-wire cache clear` | Delete the cache. |

Game names are case-insensitive. Levels accept `4`, `bandit4`, `level4` or `l4`.
`ssh` with no level defaults to level 0. Natas is web-based, so
`over-the-wire ssh natas 3` prints the level URL instead of an SSH command.

### Options

| Option | Action |
| --- | --- |
| `--offline` | Read only from the cache, never touch the network. |
| `--refresh` | Re-download pages instead of using the cache. |
| `--links` | Show link URLs after link text. |
| `-w, --width <COLS>` | Wrap at COLS columns; `0` disables wrapping. |
| `--color <WHEN>` | ANSI styling: `auto`, `always` or `never`. |

`--color` controls monochrome ANSI styling (bold, dim, italic, underline), not
hues. It honours `NO_COLOR` and `TERM=dumb`.

### Examples

```bash
over-the-wire games
over-the-wire info bandit
over-the-wire level bandit 4
over-the-wire ssh bandit 3
over-the-wire ssh bandit 3 --run
over-the-wire search cron --game bandit
over-the-wire search "hidden file" --game bandit --json
over-the-wire info semtex
over-the-wire page /rules/
over-the-wire cache fetch --all
over-the-wire --offline level bandit 30
```

`search` exits `1` when nothing matches, like `grep`; other errors also exit `1`.

## Installation

**Requirements:** Rust 1.88+ and `git`.

```bash
cargo install --git https://github.com/batusaribay/over-the-wire.git --locked
```

## Uninstall

```bash
cargo uninstall over-the-wire
rm -rf ~/.cache/over-the-wire
```

## Known Issues

- The renderer understands the current site structure. A major redesign of
  overthewire.org can change a page's layout; `page` can still render it without
  game-specific assumptions.
- Offline search only sees level pages that are in the cache.
- `--links` makes long reading lists wider, because URLs are shown inline.
- OverTheWire content is fetched live. If the site is down, only cached pages work.

## Future Ideas

- Shell completions and a man page
- Local progress notes that survive across sessions
- Search inside the included source files

## License

The `over-the-wire` code is licensed under the [MIT License](LICENSE).

The wargame notes, goals and source files shown by the tool belong to
OverTheWire and remain copyright their authors. The tool fetches them at runtime
and does not redistribute them.
