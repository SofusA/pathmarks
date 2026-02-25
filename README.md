# pathmarks
pathmarks is a tool for bookmarking paths, and a smarter way to change directory.

This is similar to other autojump tools like zoxide, but you need to manually mark bookmarks.

## Usage
Init pathmarks in your shell. Currently fish is supported.

```bash
# fish
pathmarks init fish | source
```

This will add commands `t`, `ts` and `ti` to your shell.
- `t` list stored bookmarks, picking one changed directory.
- `ts` stores current directory as a bookmark.
- `ti` interactively prompts the picker.
- `td` remove selected bookmark.
- `t <ARGUMENT>` tries to guess where you want to go. First checks case insensitive for directories. Then fussy finds in saved bookmarks.

You can provide a `--cmd` to specify the command.

You can delete bookmarks with `pathmarks delete`, and prune invalid bookmarks with `pathmarks prune`.

## Installation
### Cargo
```
cargo install pathmarks
```
