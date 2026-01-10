use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
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
    Remove { path: Option<String> },
    Prune,
    List,
    Pick,
}

fn bookmarks_file() -> AppResult<PathBuf> {
    let file = if let Some(cfg) = dirs::config_dir() {
        cfg.join("pathmarks").join("bookmarks")
    } else if let Some(home) = dirs::home_dir() {
        home.join(".bookmarks")
    } else {
        return Err(AppError::ConfigOrHomeNotFound);
    };
    Ok(file)
}

fn read_bookmarks() -> AppResult<Vec<String>> {
    let file = bookmarks_file()?;
    let f = File::open(&file)?;
    let reader = BufReader::new(f);
    let mut v = Vec::new();
    for line in reader.lines() {
        let s = line?;
        let s = s.trim().to_string();
        if !s.is_empty() {
            v.push(s);
        }
    }
    Ok(v)
}

fn write_bookmarks(v: &[String]) -> AppResult<()> {
    let file = bookmarks_file()?;
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut f = OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(file)?;
    for s in v {
        writeln!(f, "{}", s)?;
    }
    Ok(())
}

fn is_abs(p: &str) -> bool {
    Path::new(p).is_absolute()
}

fn pick_one(items: &[String]) -> AppResult<Option<String>> {
    let mut picker = Picker::new(StrRenderer);
    let injector = picker.injector();
    for s in items {
        injector.push(s.clone());
    }
    Ok(picker.pick()?.map(|s| s.to_string()))
}

fn main() -> AppResult<()> {
    let cli = Cli::parse();
    match cli.command {
        Cmd::Save => {
            let cwd = env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .map_err(AppError::Io)?;
            let mut v = match read_bookmarks() {
                Ok(v) => v,
                Err(AppError::Io(e)) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
                Err(e) => return Err(e),
            };
            if !v.iter().any(|s| s == &cwd) {
                v.push(cwd);
            }
            write_bookmarks(&v)?;
        }
        Cmd::Remove { path } => {
            let mut v = read_bookmarks()?;
            let target = if let Some(p) = path {
                if !is_abs(&p) {
                    return Err(AppError::InvalidPath);
                }
                Some(p)
            } else {
                pick_one(&v)?
            };
            if let Some(t) = target {
                let before = v.len();
                v.retain(|s| s != &t);
                if v.len() == before {
                    return Err(AppError::NotFound(t));
                }
                write_bookmarks(&v)?;
            }
        }
        Cmd::Prune => {
            let v = read_bookmarks()?;
            let before = v.len();
            let mut kept = Vec::new();
            for s in v {
                if Path::new(&s).exists() {
                    kept.push(s);
                }
            }
            let removed = before.saturating_sub(kept.len());
            write_bookmarks(&kept)?;
            println!("{}", removed);
        }
        Cmd::List => match read_bookmarks() {
            Ok(v) if !v.is_empty() => {
                for s in v {
                    println!("{}", s);
                }
            }
            Ok(_) => {
                eprintln!("No bookmarks found");
            }
            Err(AppError::Io(e)) if e.kind() == io::ErrorKind::NotFound => {
                eprintln!("No bookmarks found");
            }
            Err(e) => return Err(e),
        },
        Cmd::Pick => {
            let v = read_bookmarks()?;
            match pick_one(&v)? {
                Some(s) => println!("{}", s),
                None => std::process::exit(1),
            }
        }
    }
    Ok(())
}
