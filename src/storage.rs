// src/storage.rs
use std::fs::{OpenOptions, read_to_string};
use std::io::Write;

// 函數體：管寫入的公式
pub fn append_note(path: &str, content: &str) -> std::io::Result<()> {
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{}", content)
}

// 函數體：管讀取的公式
pub fn read_note(path: &str) -> std::io::Result<String> {
    read_to_string(path)
}

