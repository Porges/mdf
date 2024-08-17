use std::sync::Arc;

struct Sources {
    files: Vec<Arc<[u8]>>,
}

impl Sources {
    fn load_file(&mut self, path: &str) -> Result<usize, std::io::Error> {
        let data = std::fs::read(path)?;
        self.files.push(data.into());
        Ok(0)
    }
}
