use clap::ValueEnum;

#[derive(Copy, Clone, ValueEnum)]
pub enum Shell {
    Fish,
    // Zsh,
    // Bash,
    // Nushell,
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


function {command}i'
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
complete --no-files --keep-order -c {command} -a "(pathmarks list)""#,
                command = command
            )
        }
    }
}
