use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand, ValueEnum};
use nucleo_picker::nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_picker::nucleo::{Config, Matcher};
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
    Guess {
        path: String,
    },
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

fn main() {
    let cli = Cli::parse();
    let Ok(bookmark_path) = bookmarks_file() else {
        return;
    };

    match app(cli, bookmark_path) {
        Ok(res) => {
            if let Some(res) = res {
                println!("{res}")
            }
        }
        Err(err) => println!("{err}"),
    };
}

fn app(cli: Cli, bookmarks_file: PathBuf) -> AppResult<Option<String>> {
    match cli.command {
        Cmd::Save => {
            let cwd = env::current_dir()
                .map(|path| path.to_string_lossy().to_string())
                .map_err(AppError::Io)?;
            let mut bookmarks = read_bookmarks(&bookmarks_file)?;
            if !bookmarks.iter().any(|bookmark| bookmark == &cwd) {
                bookmarks.push(cwd);
            }
            write_bookmarks(&bookmarks, bookmarks_file)?;
            Ok(None)
        }
        Cmd::Remove { path } => {
            let mut bookmarks = read_bookmarks(&bookmarks_file)?;

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
                write_bookmarks(&bookmarks, bookmarks_file)?;
            }

            Ok(None)
        }
        Cmd::Guess { path } => {
            if is_absolute(&path) {
                return Ok(Some(path));
            }

            if let Some(guess) = find_case_insensitive(&path) {
                return Ok(Some(guess.to_string_lossy().into()));
            };

            let bookmarks = read_bookmarks(&bookmarks_file)?;

            if let Some((best, _score)) =
                best_bookmark_match(&path, bookmarks.iter().map(|s| s.as_str()), 100)
            {
                return Ok(Some(best.into()));
            }

            Ok(Some(path))
        }
        Cmd::Prune => {
            let bookmarks = read_bookmarks(&bookmarks_file)?;
            let mut kept = Vec::new();
            for bookmark in bookmarks {
                if Path::new(&bookmark).exists() {
                    kept.push(bookmark);
                }
            }
            write_bookmarks(&kept, bookmarks_file)?;
            Ok(None)
        }
        Cmd::List => {
            let bookmarks = read_bookmarks(&bookmarks_file)?;
            Ok(Some(bookmarks.join("\n")))
        }
        Cmd::Pick => {
            let Ok(bookmarks) = read_bookmarks(&bookmarks_file) else {
                return Ok(None);
            };

            match pick_one(&bookmarks)? {
                Some(bookmark) => Ok(Some(bookmark)),
                None => Ok(None),
            }
        }
        Cmd::Init { shell, command } => match shell {
            Shell::Fish => {
                let command = command.unwrap_or("t".to_string());

                Ok(Some(format!(
                    r#"function {command}
    if test (count $argv) -gt 0
        cd (pathmarks guess $argv[1])
        return
    end

    set p (pathmarks pick)
    test -n "$p"; and cd "$p"
end
alias ts "pathmarks save"
complete -c {command} -a "(pathmarks list)" "#,
                    command = command
                )))
            }
        },
    }
}

fn best_bookmark_match<'a>(
    query: &str,
    bookmarks: impl IntoIterator<Item = &'a str>,
    min_score: u32,
) -> Option<(&'a str, u32)> {
    let mut matcher = Matcher::new(Config::DEFAULT.match_paths());

    let results = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart)
        .match_list(bookmarks, &mut matcher);

    results
        .into_iter()
        .max_by_key(|(_, score)| *score)
        .filter(|(_, score)| *score >= min_score)
}

fn find_case_insensitive(name: &str) -> Option<PathBuf> {
    let wanted = name.to_lowercase();
    for entry in fs::read_dir(".").ok()? {
        let entry = entry.ok()?;
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();
        if file_name_str.to_lowercase() == wanted {
            return Some(PathBuf::from(file_name_str.to_string()));
        }
    }
    None
}

fn bookmarks_file() -> AppResult<PathBuf> {
    let file = if let Some(data_dir) = dirs::data_local_dir() {
        data_dir.join("pathmarks").join("bookmarks.txt")
    } else {
        return Err(AppError::DataDirectoryNotFound);
    };
    Ok(file)
}

fn read_bookmarks(file: &Path) -> AppResult<Vec<String>> {
    let file = File::open(file)?;
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

fn write_bookmarks(bookmarks: &[String], file: PathBuf) -> AppResult<()> {
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
