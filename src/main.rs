use std::fs::{self, File};
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
        paths: Vec<String>,
    },
    Pick,
    Init {
        shell: Shell,
        command: Option<String>,
    },
}

const MIN_MATCH_SCORE: u32 = 60;

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
            let cwd = env::current_dir()?.canonicalize()?;

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
                pick_one(&bookmarks)?.map(|x| x.to_string_lossy().into_owned())
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

        Cmd::Guess { paths } => {
            let Some(first) = paths.first() else {
                return Ok(None);
            };

            if is_absolute(first) {
                return Ok(Some(first.clone()));
            }

            let bookmarks = read_bookmarks(&bookmarks_file)?;
            let current_dir = env::current_dir()?;

            let mut current = match find_case_insensitive(&current_dir, first) {
                Some(path) => path,
                None => {
                    match best_bookmark_match(first, bookmarks.iter().flat_map(|s| s.to_str())) {
                        Some(bookmark) => PathBuf::from(bookmark),
                        None => return Ok(Some(paths.join("/"))),
                    }
                }
            };

            for segment in paths.iter().skip(1) {
                match find_case_insensitive(&current, segment) {
                    Some(next) => current = next,
                    None => return Ok(Some(current.join(segment).to_string_lossy().into_owned())),
                }
            }

            Ok(Some(current.to_string_lossy().into_owned()))
        }
        Cmd::Prune => {
            let bookmarks = read_bookmarks(&bookmarks_file)?;
            let kept: Vec<_> = bookmarks.into_iter().filter(|p| p.exists()).collect();

            write_bookmarks(&kept, &bookmarks_file)?;
            Ok(None)
        }
        Cmd::List => {
            let bookmarks = read_bookmarks(&bookmarks_file)?;
            let current_dir = env::current_dir()?;
            let out = map_relative_paths(&current_dir, bookmarks);

            let out: Vec<_> = out
                .into_iter()
                .map(|x| x.to_string_lossy().into_owned())
                .collect();

            Ok(Some(out.join("\n")))
        }
        Cmd::Pick => {
            let bookmarks = read_bookmarks(&bookmarks_file)?;
            let current_dir = env::current_dir()?;

            let relative_bookmarks = map_relative_paths(&current_dir, bookmarks);
            let sub_directories = list_child_dirs(&current_dir, false)?;
            let mut relative_sub_directories = map_relative_paths(&current_dir, sub_directories);
            relative_sub_directories.push(PathBuf::from(".."));

            match pick_one_last_dim(&relative_sub_directories, &relative_bookmarks)? {
                Some(bookmark) => Ok(bookmark.to_str().map(|x| x.into())),
                None => Ok(None),
            }
        }
        Cmd::Init { shell, command } => Ok(Some(init(shell, command))),
    }
}

fn best_match<'a, I>(query: &str, items: I) -> Option<(&'a str, u32)>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut matcher = Matcher::new(Config::DEFAULT.match_paths());

    Pattern::parse(query, CaseMatching::Smart, Normalization::Smart)
        .match_list(items, &mut matcher)
        .into_iter()
        .filter(|(_, score)| *score >= MIN_MATCH_SCORE)
        .max_by(|(a_str, a_score), (b_str, b_score)| {
            a_score
                .cmp(b_score)
                .then_with(|| b_str.len().cmp(&a_str.len()))
        })
}

fn best_bookmark_match<'a>(
    query: &str,
    bookmarks: impl IntoIterator<Item = &'a str>,
) -> Option<&'a str> {
    best_match(query, bookmarks).map(|(s, _)| s)
}

fn find_fuzzy(root: &Path, query: &str) -> Option<PathBuf> {
    let dir_names: Vec<String> = fs::read_dir(root)
        .ok()?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            if entry.file_type().ok()?.is_dir() {
                Some(entry.file_name().to_string_lossy().into_owned())
            } else {
                None
            }
        })
        .collect();

    best_match(query, dir_names.iter().map(String::as_str)).map(|(name, _)| root.join(name))
}

fn find_case_insensitive(root: &Path, query: &str) -> Option<PathBuf> {
    if !query.contains('/')
        && let Some(fuzzy) = find_fuzzy(root, query)
    {
        return Some(fuzzy);
    }

    let mut current = root.to_path_buf();

    for wanted in query.trim_end_matches('/').split('/') {
        let wanted = wanted.to_lowercase();

        let mut matched = None;

        for entry in fs::read_dir(&current).ok()? {
            let entry = entry.ok()?;

            if !entry.file_type().ok()?.is_dir() {
                continue;
            }

            let name = entry.file_name();

            if name.to_string_lossy().to_lowercase() == wanted {
                matched = Some(entry.path());
                break;
            }
        }

        current = matched?;
    }

    Some(current)
}

fn bookmarks_file() -> AppResult<PathBuf> {
    let file = dirs::data_local_dir()
        .ok_or(AppError::DataDirectoryNotFound)?
        .join("pathmarks")
        .join("bookmarks.txt");

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
        let line = line.trim();
        if !line.is_empty() {
            bookmarks.push(PathBuf::from(line));
        }
    }
    Ok(bookmarks)
}

fn write_bookmarks(bookmarks: &[PathBuf], file: &Path) -> AppResult<()> {
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent)?;
    }

    let tmp = file.with_extension("tmp");

    {
        let mut out = File::create(&tmp)?;

        for bookmark in bookmarks {
            writeln!(out, "{}", bookmark.display())?;
        }

        out.flush()?;
    }

    fs::rename(tmp, file)?;

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
        .filter(|p| p.to_str() != Some("."))
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

    #[test]
    fn test_find_case_insensitive_nested() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();

        let dir_path = root.join("Dir");
        let subdir_path = dir_path.join("SubDir");

        fs::create_dir_all(&subdir_path).unwrap();

        let found = find_case_insensitive(root, "dIr/sUbDiR").unwrap();

        assert_eq!(found, subdir_path);
    }

    #[test]
    fn test_find_case_insensitive_fuzzy() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();

        let dir_1 = root.join("Test_Project");
        let dir_2 = root.join("other_directory");

        fs::create_dir_all(&dir_1).unwrap();
        fs::create_dir_all(&dir_2).unwrap();

        let found = find_case_insensitive(root, "tesproj").unwrap();

        assert_eq!(found, dir_1);
    }

    #[test]
    fn test_find_case_insensitive_not_fuzzy_sub() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();

        let dir_path = root.join("Dir");
        let subdir_path = dir_path.join("SubDirectory");

        fs::create_dir_all(&subdir_path).unwrap();

        let found = find_case_insensitive(root, "subdir");

        assert_eq!(found, None);
    }

    #[test]
    fn test_find_case_insensitive_not_files() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();

        let dir_path = root.join("Dir");

        fs::create_dir_all(&dir_path).unwrap();

        let file_path = dir_path.join("testfile.txt");
        fs::write(&file_path, "hello").unwrap();

        let found = find_case_insensitive(root, "testf");

        assert_eq!(found, None);
    }

    #[test]
    fn shortest_path_wins_when_scores_equal() {
        let paths = [
            "/path/with/many/sub/directories",
            "/path/with/",
            "/path/with/many/sub/",
        ];

        let best = best_bookmark_match("pathwith", paths).unwrap();

        assert_eq!(best, "/path/with/");
    }

    #[test]
    fn best_match_returns_score_and_value() {
        let items = ["foobar", "foo", "bar"];

        let result = best_match("foo", items).unwrap();

        assert_eq!(result.0, "foo");
    }

    #[test]
    fn write_bookmarks_roundtrip() {
        let dir = tempfile::tempdir().unwrap();

        let file = dir.path().join("bookmarks.txt");

        let bookmarks = vec![PathBuf::from("/tmp/a"), PathBuf::from("/tmp/b")];

        write_bookmarks(&bookmarks, &file).unwrap();

        let loaded = read_bookmarks(&file).unwrap();

        assert_eq!(loaded, bookmarks);
    }

    #[test]
    fn nested_query_does_not_use_root_fuzzy_match() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();

        fs::create_dir_all(root.join("Project")).unwrap();
        fs::create_dir_all(root.join("Dir").join("SubDir")).unwrap();

        let found = find_case_insensitive(root, "dir/subdir").unwrap();

        assert_eq!(found, root.join("Dir").join("SubDir"));
    }

    #[test]
    fn canonicalized_paths_deduplicate() {
        let temp = tempfile::tempdir().unwrap();

        let canonical = temp.path().canonicalize().unwrap();

        let alternative = canonical.join("..").join(canonical.file_name().unwrap());

        assert_eq!(canonical, alternative.canonicalize().unwrap());
    }

    #[test]
    fn test_find_case_insensitive_unicode() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();

        let dir = root.join("Risengrød");

        fs::create_dir_all(&dir).unwrap();

        let found = find_case_insensitive(root, "risengrød").unwrap();

        assert_eq!(found, dir);
    }

    #[test]
    fn test_find_case_insensitive_unicode_nested() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();

        let subdir = root.join("Rød").join("Grød");

        fs::create_dir_all(&subdir).unwrap();

        let found = find_case_insensitive(root, "rød/grød").unwrap();

        assert_eq!(found, subdir);
    }
}
