# over-the-wire

A Rust CLI for OverTheWire games.

![over-the-wire in a terminal](preview.png)

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
6. Output is plain text. Every command that makes sense as data has a `--json`
   form.

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
| `over-the-wire rules` | Show the game rules. |
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

## Install

**Requirements:** Rust 1.88+ and `git`.

```bash
cargo install --git https://github.com/batusaribay/over-the-wire.git --locked
```

## Uninstall

```bash
cargo uninstall over-the-wire
rm -rf ~/.cache/over-the-wire
```

## License

The `over-the-wire` code is licensed under the [MIT License](LICENSE).

The game notes, goals and source files shown by the tool belong to
OverTheWire and remain copyright their authors. The tool fetches them at runtime
and does not redistribute them.
