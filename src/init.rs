use clap::ValueEnum;

#[derive(Copy, Clone, ValueEnum)]
pub enum Shell {
    Fish,
    Zsh,
    Bash,
    Nushell,
}

pub fn init(shell: Shell, command: Option<String>) -> String {
    match shell {
        Shell::Fish => {
            let command = command.unwrap_or("t".to_string());

            format!(
                r#"function {command}
    if test (count $argv) -gt 0
        cd (pathmarks guess $argv[1])
        return
    end

    set p (pathmarks pick)
    test -n "$p"; and cd "$p"
end
alias {command}s "pathmarks save"
complete -c {command} -a "(pathmarks list)" "#,
                command = command
            )
        }

        Shell::Zsh => {
            let command = command.unwrap_or("t".to_string());
            format!(
                r#"{command}() {{
  if [[ $# -gt 0 ]]; then
    cd "$(pathmarks guess "$1")"
    return
  fi

  local p
  p="$(pathmarks pick)"
  if [[ -n "$p" ]]; then cd "$p"; fi
}}

alias {command}s="pathmarks save"

# Completion: suggest names from `pathmarks list`
_{command}() {{
  compadd -- $(pathmarks list)
}}
compdef _{command} {command}
"#,
                command = command
            )
        }

        Shell::Bash => {
            let command = command.unwrap_or("t".to_string());
            format!(
                r#"{command}() {{
  if [[ $# -gt 0 ]]; then
    cd "$(pathmarks guess "$1")"
    return
  fi

  local p
  p="$(pathmarks pick)"
  if [[ -n "$p" ]]; then cd "$p"; fi
}}

alias {command}s="pathmarks save"

# Completion: suggest names from `pathmarks list`
_{command}_complete() {{
  local cur
  cur="${{COMP_WORDS[COMP_CWORD]}}"
  COMPREPLY=( $(compgen -W "$(pathmarks list)" -- "$cur") )
}}
complete -F _{command}_complete {command}
"#,
                command = command
            )
        }

        Shell::Nushell => {
            let command = command.unwrap_or("t".to_string());
            format!(
                r#"def pathmarks_completer [] {{
  pathmarks list | lines
}}

export def-env {command} [name?: string@pathmarks_completer] {{
  if $name != null {{
    let d = (pathmarks guess $name | str trim)
    if $d != "" {{
      cd $d
    }}
    return
  }}

  let p = (pathmarks pick | str trim)
  if $p != "" {{
    cd $p
  }}
}}

export alias {command}s = pathmarks save
"#,
                command = command
            )
        }
    }
}
