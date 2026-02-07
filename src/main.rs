use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::{env, io};

use clap::{Parser, Subcommand};
use nucleo_picker::nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_picker::nucleo::{Config, Matcher};

use crate::error::{AppError, AppResult};
use crate::init::{Shell, init};
use crate::pickers::{pick_one, pick_one_last_dim};

mod error;
mod index_renderer;
mod init;
mod pickers;

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
            let cwd = env::current_dir().map_err(AppError::Io)?;
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
                pick_one(&bookmarks)?.and_then(|x| x.to_str().map(|x| x.to_string()))
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
                return Ok(guess.to_str().map(|x| x.into()));
            };

            let bookmarks = read_bookmarks(&bookmarks_file)?;

            if let Some(best) =
                best_bookmark_match(&path, bookmarks.iter().flat_map(|s| s.to_str()))
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
            write_bookmarks(&kept, &bookmarks_file)?;
            Ok(None)
        }
        Cmd::List => {
            let out = merged_directories(bookmarks_file)?;
            let out: Vec<_> = out
                .into_iter()
                .flat_map(|x| x.to_str().map(|x| x.to_string()))
                .collect();

            Ok(Some(out.join("\n")))
        }
        Cmd::Pick => {
            let bookmarks = read_bookmarks(&bookmarks_file)?;
            let current_dir = env::current_dir()?;

            let relative_bookmarks = map_relative_paths(&current_dir, bookmarks);
            let sub_directories = list_child_dirs(&current_dir, false)?;
            let relative_sub_directories = map_relative_paths(&current_dir, sub_directories);

            match pick_one_last_dim(&relative_sub_directories, &relative_bookmarks)? {
                Some(bookmark) => Ok(bookmark.to_str().map(|x| x.into())),
                None => Ok(None),
            }
        }
        Cmd::Init { shell, command } => Ok(Some(init(shell, command))),
    }
}

fn merged_directories(bookmarks_file: PathBuf) -> AppResult<Vec<PathBuf>> {
    let bookmarks = read_bookmarks(&bookmarks_file)?;
    let merged_directories = merge_with_cwd_dirs(bookmarks)?;

    let cwd = env::current_dir()?;

    Ok(map_relative_paths(&cwd, merged_directories))
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
        let file_name_str = file_name.to_str()?;
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

fn read_bookmarks(file: &Path) -> AppResult<Vec<PathBuf>> {
    let file = File::open(file)?;
    let reader = BufReader::new(file);
    let mut bookmarks = Vec::new();
    for line in reader.lines() {
        let line = line?;
        let line = line.trim().to_string();
        if !line.is_empty() {
            bookmarks.push(PathBuf::from(line));
        }
    }
    Ok(bookmarks)
}

fn write_bookmarks(bookmarks: &[PathBuf], file: &PathBuf) -> AppResult<()> {
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(file)?;

    for bookmark in bookmarks.iter().flat_map(|x| x.to_str()) {
        writeln!(file, "{}", bookmark)?;
    }
    Ok(())
}

fn is_absolute(p: &str) -> bool {
    Path::new(p).is_absolute()
}

fn list_child_dirs(dir: &Path, include_hidden: bool) -> io::Result<Vec<PathBuf>> {
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
            out.push(entry.path());
        }
    }

    out.sort_unstable();
    Ok(out)
}

fn merge_with_cwd_dirs(paths: Vec<PathBuf>) -> io::Result<Vec<PathBuf>> {
    let current_dir = env::current_dir()?;
    let current_dir_sub_dirs = list_child_dirs(&current_dir, false)?;

    let capacity = paths.len() + current_dir_sub_dirs.len();
    let mut seen = HashSet::with_capacity(capacity);
    let mut merged = Vec::with_capacity(capacity);

    for directory in current_dir_sub_dirs.into_iter().chain(paths) {
        if !seen.contains(&directory) {
            seen.insert(directory.clone());
            merged.push(directory);
        }
    }

    Ok(merged)
}

fn relative_if_descendant(base: &Path, child: &Path) -> Option<PathBuf> {
    if !base.is_absolute() || !child.is_absolute() {
        return None;
    }
    if !child.starts_with(base) {
        return None;
    }

    child
        .strip_prefix(base)
        .map(|rel| {
            if rel.as_os_str().is_empty() {
                PathBuf::from(".")
            } else {
                rel.to_path_buf()
            }
        })
        .ok()
}

fn map_relative_paths<I>(base: &Path, paths: I) -> Vec<PathBuf>
where
    I: IntoIterator<Item = PathBuf>,
{
    paths
        .into_iter()
        .map(|p| relative_if_descendant(base, &p).unwrap_or(p))
        .collect()
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
