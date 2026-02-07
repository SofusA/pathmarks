use std::path::PathBuf;

use nucleo_picker::{Picker, Render};

use crate::{error::AppResult, index_renderer::IndexPathRenderer};

pub fn pick_one(bookmarks: &[PathBuf]) -> AppResult<Option<&PathBuf>> {
    let mut picker = Picker::new(IndexPathRenderer::new(bookmarks));
    let mut injector = picker.injector();
    injector.extend(0..bookmarks.len());

    let selected_idx = picker.pick()?.copied();

    Ok(selected_idx.map(|i| &bookmarks[i]))
}

#[derive(Clone, Copy)]
enum Source {
    First,
    Second,
}

struct Entry<'a> {
    path: &'a PathBuf,
    source: Source,
}

pub fn pick_one_last_dim<'a>(
    first: &'a [PathBuf],
    second: &'a [PathBuf],
) -> AppResult<Option<&'a PathBuf>> {
    let entries: Vec<Entry<'a>> = first
        .iter()
        .map(|p| Entry {
            path: p,
            source: Source::First,
        })
        .chain(second.iter().map(|p| Entry {
            path: p,
            source: Source::Second,
        }))
        .collect();

    let renderer = DualListIndexRenderer { entries: &entries };

    let mut picker = Picker::new(renderer);
    let mut injector = picker.injector();

    injector.extend(0..entries.len());

    let selected_idx = picker.pick()?.copied();
    Ok(selected_idx.map(|i| entries[i].path))
}

pub struct DualListIndexRenderer<'a> {
    entries: &'a [Entry<'a>],
}

impl<'a> Render<usize> for DualListIndexRenderer<'a> {
    type Str<'b>
        = String
    where
        usize: 'b;

    fn render<'b>(&self, idx: &'b usize) -> Self::Str<'b> {
        let entry = &self.entries[*idx];
        let path = entry.path.to_string_lossy();

        const ITALIC: &str = "\x1b[3m";
        const DIM: &str = "\x1b[2m";
        const RESET: &str = "\x1b[0m";

        match entry.source {
            Source::First => path.to_string(),
            Source::Second => format!("{DIM}{ITALIC}{path}{RESET}"),
        }
    }
}
