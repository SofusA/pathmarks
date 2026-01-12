use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand, ValueEnum};
use nucleo_picker::{Picker, render::StrRenderer};

use crate::error::{AppError, AppResult};

mod error;

#[derive(Parser)]
#[command(name = "pathmarks")]
#[command(about = "Path bookmark manager", version)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    Save,
    Remove {
        path: Option<String>,
    },
    Prune,
    List,
    Pick,
    Init {
        shell: Shell,
        command: Option<String>,
    },
}

#[derive(Copy, Clone, ValueEnum)]
enum Shell {
    Fish,
}

fn bookmarks_file() -> AppResult<PathBuf> {
    let file = if let Some(data_dir) = dirs::data_local_dir() {
        data_dir.join("pathmarks").join("bookmarks.txt")
    } else {
        return Err(AppError::DataDirectoryNotFound);
    };
    Ok(file)
}

fn read_bookmarks() -> AppResult<Vec<String>> {
    let file = bookmarks_file()?;
    let file = File::open(&file)?;
    let reader = BufReader::new(file);
    let mut bookmarks = Vec::new();
    for line in reader.lines() {
        let line = line?;
        let line = line.trim().to_string();
        if !line.is_empty() {
            bookmarks.push(line);
        }
    }
    Ok(bookmarks)
}

fn write_bookmarks(bookmarks: &[String]) -> AppResult<()> {
    let file = bookmarks_file()?;
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(file)?;
    for bookmark in bookmarks {
        writeln!(file, "{}", bookmark)?;
    }
    Ok(())
}

fn is_absolute(p: &str) -> bool {
    Path::new(p).is_absolute()
}

fn pick_one(bookmarks: &[String]) -> AppResult<Option<String>> {
    let mut picker = Picker::new(StrRenderer);
    let injector = picker.injector();
    for bookmark in bookmarks {
        injector.push(bookmark.clone());
    }
    Ok(picker.pick()?.map(|bookmark| bookmark.to_string()))
}

fn main() -> AppResult<()> {
    let cli = Cli::parse();
    match cli.command {
        Cmd::Save => {
            let cwd = env::current_dir()
                .map(|path| path.to_string_lossy().to_string())
                .map_err(AppError::Io)?;
            let mut bookmarks = match read_bookmarks() {
                Ok(bookmarks) => bookmarks,
                Err(AppError::Io(error)) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
                Err(error) => return Err(error),
            };
            if !bookmarks.iter().any(|bookmark| bookmark == &cwd) {
                bookmarks.push(cwd);
            }
            write_bookmarks(&bookmarks)?;
        }
        Cmd::Remove { path } => {
            let mut bookmarks = read_bookmarks()?;
            let target = if let Some(path) = path {
                if !is_absolute(&path) {
                    return Err(AppError::InvalidPath);
                }
                Some(path)
            } else {
                pick_one(&bookmarks)?
            };
            if let Some(target) = target {
                let before = bookmarks.len();
                bookmarks.retain(|s| s != &target);
                if bookmarks.len() == before {
                    return Err(AppError::NotFound(target));
                }
                write_bookmarks(&bookmarks)?;
            }
        }
        Cmd::Prune => {
            let bookmarks = read_bookmarks()?;
            let before = bookmarks.len();
            let mut kept = Vec::new();
            for bookmark in bookmarks {
                if Path::new(&bookmark).exists() {
                    kept.push(bookmark);
                }
            }
            let removed = before.saturating_sub(kept.len());
            write_bookmarks(&kept)?;
            println!("{}", removed);
        }
        Cmd::List => match read_bookmarks() {
            Ok(bookmarks) if !bookmarks.is_empty() => {
                for bookmark in bookmarks {
                    println!("{}", bookmark);
                }
            }
            Ok(_) => {
                eprintln!("No bookmarks found");
            }
            Err(AppError::Io(error)) if error.kind() == io::ErrorKind::NotFound => {
                eprintln!("No bookmarks found");
            }
            Err(error) => return Err(error),
        },
        Cmd::Pick => {
            let bookmarks = read_bookmarks()?;
            match pick_one(&bookmarks)? {
                Some(bookmark) => println!("{}", bookmark),
                None => std::process::exit(1),
            }
        }
        Cmd::Init { shell, command } => match shell {
            Shell::Fish => {
                let command = command.unwrap_or("t".to_string());

                println!(
                    r#"function {command}
    if test (count $argv) -gt 0
        cd "$argv[1]"
        return
    end

    set p (pathmarks pick)
    test -n "$p"; and cd "$p"
end
alias ts "pathmarks save"
complete -c {command} -a "(pathmarks list)" "#,
                    command = command
                );
            }
        },
    }
    Ok(())
}
