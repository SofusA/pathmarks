# pathmarks
pathmarks is a tool for bookmarking paths, and a smarter way to change directory.

This is similar to other autojump tools like zoxide, but you need to manually mark bookmarks.

## Usage
Init pathmarks in your shell. Fish is supported, while zsh, bash and nushell is experimental.

```bash
# fish
pathmarks init fish | source

# zsh
eval "$(pathmarks init zsh)"

# bash
eval "$(pathmarks init bash)"

# nushell
pathmarks init nushell | save ~/.nu-pathmarks.nu
source ~/.nu-pathmarks.nu
```

This will add commands `t`, `ts` and `ti` to your shell.
`t` list saved bookmarks, picking one changed directory.
`ts` saves given bookmark.
`ti` interactively prompts the picker.
`t <ARGUMENT>` tries to guess where you want to go. First checks case insensitive for directories. Then fussy finds in saved bookmarks.

You can provide a `--cmd` to specify the command.

You can delete bookmarks with `pathmarks delete`, and prune invalid bookmarks with `pathmarks prune`.

## Installation
### Cargo
```
cargo install pathmarks
```
