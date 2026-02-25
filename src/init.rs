use clap::ValueEnum;

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum Shell {
    Fish,
    // Zsh,
    // Bash,
    // Nu,
}

pub fn init(shell: Shell, command: Option<String>) -> String {
    let command = command.unwrap_or_else(|| "t".to_string());
    match shell {
        Shell::Fish => fish_init(&command),
        // Shell::Zsh => zsh_init(&command),
        // Shell::Bash => bash_init(&command),
        // Shell::Nu => nu_init(&command),
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
complete --keep-order -c {command} -d "Pathmarks" --wraps cd -a "(pathmarks list)"
"#
    )
}
