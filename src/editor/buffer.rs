use anyhow::Result;
use ropey::Rope;
use std::fs;
use std::path::Path;

pub struct EditBuffer {
    pub rope: Rope,
}

impl EditBuffer {
    pub fn new() -> Self {
        Self { rope: Rope::new() }
    }

    pub fn from_str(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
        }
    }

    pub fn load_file<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let file = fs::File::open(path)?;
        self.rope = Rope::from_reader(std::io::BufReader::new(file))?;
        Ok(())
    }

    pub fn save_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let file = fs::File::create(path)?;
        self.rope.write_to(std::io::BufWriter::new(file))?;
        Ok(())
    }


    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    pub fn insert(&mut self, char_idx: usize, text: &str) {
        if char_idx <= self.rope.len_chars() {
            self.rope.insert(char_idx, text);
        }
    }

    pub fn remove(&mut self, start_idx: usize, end_idx: usize) {
        let len = self.rope.len_chars();
        let s = start_idx.min(len);
        let e = end_idx.min(len);
        if s < e {
            self.rope.remove(s..e);
        }
    }
}

impl std::fmt::Display for EditBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.rope)
    }
}
