// src/storage.rs
use std::fs::{write, read_to_string};

// 將完整的加密內容覆蓋寫入本地檔案
pub fn write_encrypted_note(path: &str, encrypted_content: &str) -> std::io::Result<()> {
    write(path, encrypted_content)
}

// 管讀取本地檔案的公式
pub fn read_note(path: &str) -> std::io::Result<String> {
    read_to_string(path)
}