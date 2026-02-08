use clap::ValueEnum;

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum Shell {
    Fish,
    Zsh,
    Bash,
    Nushell,
}

pub fn init(shell: Shell, command: Option<String>) -> String {
    let command = command.unwrap_or_else(|| "t".to_string());
    match shell {
        Shell::Fish => fish_init(&command),
        Shell::Zsh => zsh_init(&command),
        Shell::Bash => bash_init(&command),
        Shell::Nushell => nushell_init(&command),
    }
}

fn fish_init(command: &str) -> String {
    format!(
        r#"function {command}
    if test (count $argv) -gt 0
        cd (pathmarks guess $argv[1])
        return
    end

    set p (pathmarks pick)
    test -n "$p"; and cd "$p"
end

function {command}i
    while true
        set -l dest (pathmarks pick)
        set -l code $status

        if test $code -ne 0; or test -z "$dest"
            break
        end

        if test -d "$dest"
            cd "$dest"
        else
            break
        end
    end
end

alias {command}s "pathmarks save"
alias {command}d "pathmarks remove"
complete --no-files --keep-order -c {command} -a "(pathmarks list)"
"#
    )
}

fn zsh_init(command: &str) -> String {
    format!(
        r#"{command}() {{
  if [[ $# -gt 0 ]]; then
    cd "$(pathmarks guess "$1")"
    return
  fi
  local p
  p="$(pathmarks pick)"
  [[ -n "$p" ]] && cd "$p"
}}

{command}i() {{
  while true; do
    local dest
    dest="$(pathmarks pick)"
    local code=$?
    if [[ $code -ne 0 || -z "$dest" ]]; then
      break
    fi
    if [[ -d "$dest" ]]; then
      cd "$dest"
    else
      break
    fi
  done
}}

alias {command}s='pathmarks save'
alias {command}d='pathmarks remove'

# Completion: compdef + helper that feeds candidates from `pathmarks list`
_{command}() {{
  local -a candidates
  candidates=($(pathmarks list 2>/dev/null))
  compadd -a candidates
}}
compdef _{command} {command}
"#
    )
}

fn bash_init(command: &str) -> String {
    format!(
        r#"{command}() {{
  if [[ $# -gt 0 ]]; then
    cd "$(pathmarks guess "$1")"
    return
  fi
  local p
  p="$(pathmarks pick)"
  [[ -n "$p" ]] && cd "$p"
}}

{command}i() {{
  while true; do
    local dest
    dest="$(pathmarks pick)"
    local code=$?
    if [[ $code -ne 0 || -z "$dest" ]]; then
      break
    fi
    if [[ -d "$dest" ]]; then
      cd "$dest"
    else
      break
    fi
  done
}}

alias {command}s='pathmarks save'
alias {command}d='pathmarks remove'

_{command}_complete() {{
  local cur
  cur="${{COMP_WORDS[COMP_CWORD]}}"
  # Collect candidates from `pathmarks list`
  COMPREPLY=($(compgen -W "$(pathmarks list 2>/dev/null)" -- "$cur"))
}}
complete -o filenames -F _{command}_complete {command}
"#
    )
}

fn nushell_init(command: &str) -> String {
    format!(
        r#"export def "nu-complete pathmarks" [] {{
  pathmarks list | lines
}}

export def --env {command} [name?: string@"nu-complete pathmarks"] {{
  if $name != null {{
    cd (pathmarks guess $name)
  }} else {{
    let p = (pathmarks pick)
    if $p != "" {{
      cd $p
    }}
  }}
}}

export def --env {command}i [] {{
  loop {{
    let dest = (pathmarks pick)
    let code = $env.LAST_EXIT_CODE
    if $code != 0 or ($dest | is-empty) {{
      break
    }}
    if ($dest | path exists) and (($dest | path type) == "dir") {{
      cd $dest
    }} else {{
      break
    }}
  }}
}}

export alias {command}s = pathmarks save
"#
    )
}
