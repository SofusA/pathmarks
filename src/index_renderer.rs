use std::path::PathBuf;

pub struct IndexPathRenderer<'a> {
    items: &'a [PathBuf],
}

impl<'a> IndexPathRenderer<'a> {
    pub fn new(items: &'a [PathBuf]) -> Self {
        Self { items }
    }
}

impl<'a> nucleo_picker::Render<usize> for IndexPathRenderer<'a> {
    type Str<'b>
        = String
    where
        usize: 'b;

    fn render<'b>(&self, idx: &'b usize) -> Self::Str<'b> {
        let path = &self.items[*idx];
        path.to_string_lossy().to_string()
    }
}
