use std::collections::HashSet;
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use nucleo_picker::nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_picker::nucleo::{Config, Matcher};
use nucleo_picker::{Picker, render::StrRenderer};

use crate::error::{AppError, AppResult};
use crate::init::{Shell, init};

mod error;
mod init;

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
            write_bookmarks(&bookmarks, &bookmarks_file)?;
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
                write_bookmarks(&bookmarks, &bookmarks_file)?;
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

            if let Some(best) = best_bookmark_match(&path, bookmarks.iter().map(|s| s.as_str())) {
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
            write_bookmarks(&kept, &bookmarks_file)?;
            Ok(None)
        }
        Cmd::List => {
            let out = merged_directories(bookmarks_file)?;
            Ok(Some(out.join("\n")))
        }
        Cmd::Pick => {
            let directories = merged_directories(bookmarks_file)?;

            match pick_one(&directories)? {
                Some(bookmark) => Ok(Some(bookmark)),
                None => Ok(None),
            }
        }
        Cmd::Init { shell, command } => Ok(Some(init(shell, command))),
    }
}

fn merged_directories(bookmarks_file: PathBuf) -> AppResult<Vec<String>> {
    let bookmarks: Vec<String> = read_bookmarks(&bookmarks_file)?;
    let merged_directories = merge_with_cwd_dirs(bookmarks)?;

    let cwd = env::current_dir()?;
    let mut out = Vec::with_capacity(merged_directories.len());

    for path_string in merged_directories {
        let path = Path::new(&path_string);
        if let Some(relative) = relative_if_descendant(&cwd, path) {
            if let Some(s) = relative.to_str() {
                if s != "." {
                    out.push(s.to_string());
                }
            } else {
                out.push(path_string);
            }
        } else {
            out.push(path_string);
        }
    }

    Ok(out)
}

fn best_bookmark_match<'a>(
    query: &str,
    bookmarks: impl IntoIterator<Item = &'a str>,
) -> Option<&'a str> {
    let min_score = 100;
    let mut matcher = Matcher::new(Config::DEFAULT.match_paths());

    let results = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart)
        .match_list(bookmarks, &mut matcher);

    results
        .into_iter()
        .filter(|(_, score)| *score >= min_score)
        .max_by(|(a_str, a_score), (b_str, b_score)| {
            a_score
                .cmp(b_score)
                .then_with(|| b_str.len().cmp(&a_str.len()))
        })
        .map(|(s, _)| s)
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

    if !file.exists() {
        write_bookmarks(&[], &file)?;
    }

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

fn write_bookmarks(bookmarks: &[String], file: &PathBuf) -> AppResult<()> {
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

fn list_child_dirs(dir: &Path, include_hidden: bool) -> std::io::Result<Vec<String>> {
    let mut out = Vec::new();

    for entry_res in fs::read_dir(dir)? {
        let entry = entry_res?;
        let file_type = entry.file_type()?;

        let is_dir = if file_type.is_symlink() {
            let target = fs::read_link(entry.path())?;
            let target_abs = if target.is_absolute() {
                target
            } else {
                dir.join(target)
            };
            target_abs.is_dir()
        } else {
            file_type.is_dir()
        };

        if !is_dir {
            continue;
        }

        if let Some(name) = entry.file_name().to_str() {
            if !include_hidden && name.starts_with('.') {
                continue;
            }
            out.push(name.to_string());
        }
    }

    out.sort_unstable();
    Ok(out)
}

fn merge_with_cwd_dirs(paths: Vec<String>) -> std::io::Result<Vec<String>> {
    let cwd = env::current_dir()?;
    let cwd_dirs = list_child_dirs(&cwd, false)?;
    let mut seen: HashSet<String> = HashSet::with_capacity(paths.len() + cwd_dirs.len());
    let mut merged = Vec::with_capacity(paths.len() + cwd_dirs.len());

    for directory in cwd_dirs {
        if seen.insert(directory.clone()) {
            merged.push(directory);
        }
    }

    for path in paths {
        if seen.insert(path.clone()) {
            merged.push(path);
        }
    }

    Ok(merged)
}

fn relative_if_descendant(base: &Path, child: &Path) -> Option<PathBuf> {
    let (base_abs, child_abs) = match (base.canonicalize(), child.canonicalize()) {
        (Ok(b), Ok(c)) => (b, c),
        _ => return best_effort_relative_if_descendant(base, child),
    };

    if !child_abs.starts_with(&base_abs) {
        return None;
    }
    child_abs.strip_prefix(&base_abs).ok().map(|rel| {
        if rel.as_os_str().is_empty() {
            PathBuf::from(".")
        } else {
            rel.to_path_buf()
        }
    })
}

fn best_effort_relative_if_descendant(base: &Path, child: &Path) -> Option<PathBuf> {
    if !base.is_absolute() || !child.is_absolute() {
        return None;
    }
    if !child.starts_with(base) {
        return None;
    }
    child.strip_prefix(base).ok().map(|rel| {
        if rel.as_os_str().is_empty() {
            PathBuf::from(".")
        } else {
            rel.to_path_buf()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn best_with_same_score() {
        let paths = [
            "/path/with/many/sub/directories",
            "/path/with/",
            "/path/with/many/sub/",
        ];

        let best = best_bookmark_match("pathwith", paths).unwrap();

        assert_eq!(best, paths[1]);
    }
}
