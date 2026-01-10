use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use nucleo_picker::{Picker, render::StrRenderer};

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

fn bookmarks_file() -> io::Result<PathBuf> {
    let file = if let Some(cfg) = dirs::config_dir() {
        cfg.join("pathmarks").join("bookmarks")
    } else if let Some(home) = dirs::home_dir() {
        home.join(".bookmarks")
    } else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Could not determine home/config directory",
        ));
    };
    Ok(file)
}

fn read_bookmarks() -> io::Result<Vec<String>> {
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

fn write_bookmarks(v: &[String]) -> io::Result<()> {
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

fn pick_one(items: &[String]) -> io::Result<Option<String>> {
    let mut picker = Picker::new(StrRenderer);
    let injector = picker.injector();
    for s in items {
        injector.push(s.clone());
    }
    picker
        .pick()
        .map(|opt| opt.map(|s| s.to_string()))
        .map_err(io::Error::other) // TODO
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Cmd::Save => {
            let cwd = env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| {
                    eprintln!("Failed to get current directory");
                    std::process::exit(1);
                });
            let mut v = match read_bookmarks() {
                Ok(v) => v,
                Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
                Err(e) => {
                    eprintln!("{}", e);
                    std::process::exit(1);
                }
            };
            if !v.iter().any(|s| s == &cwd) {
                v.push(cwd);
            }
            if let Err(e) = write_bookmarks(&v) {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        Cmd::Remove { path } => {
            let mut v = match read_bookmarks() {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("{}", e);
                    std::process::exit(1);
                }
            };
            let target = if let Some(p) = path {
                if !is_abs(&p) {
                    eprintln!("Path must be absolute");
                    std::process::exit(1);
                }
                Some(p)
            } else {
                match pick_one(&v) {
                    Ok(opt) => opt,
                    Err(e) => {
                        eprintln!("{}", e);
                        std::process::exit(1);
                    }
                }
            };
            if let Some(t) = target {
                let before = v.len();
                v.retain(|s| s != &t);
                if v.len() == before {
                    eprintln!("Not found");
                    std::process::exit(2);
                }
                if let Err(e) = write_bookmarks(&v) {
                    eprintln!("{}", e);
                    std::process::exit(1);
                }
                println!("{}", t);
            }
        }

        Cmd::Prune => {
            let v = read_bookmarks().unwrap_or_else(|e| {
                eprintln!("{}", e);
                std::process::exit(1)
            });
            let before = v.len();
            let mut kept = Vec::new();
            for s in v {
                if Path::new(&s).exists() {
                    kept.push(s);
                }
            }
            let removed = before.saturating_sub(kept.len());
            if let Err(e) = write_bookmarks(&kept) {
                eprintln!("{}", e);
                std::process::exit(1);
            }
            println!("{}", removed);
        }

        Cmd::List => match read_bookmarks() {
            Ok(v) if !v.is_empty() => {
                for s in v {
                    println!("{}", s);
                }
            }
            Ok(_) => eprintln!("No bookmarks found"),
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        },

        Cmd::Pick => {
            let v = match read_bookmarks() {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("{}", e);
                    std::process::exit(1);
                }
            };
            match pick_one(&v) {
                Ok(Some(s)) => println!("{}", s),
                Ok(None) => std::process::exit(1),
                Err(e) => {
                    eprintln!("{}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}
