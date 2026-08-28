// src/encrypt.rs
use std::process::{Command, Stdio};
use std::io::Write;
use std::thread;

// 🔐 調用本地 GPG 密鑰進行動態物理加密（並發線程寫入，防 64KB 管道死鎖）
pub fn encrypt_with_gpg(data: &[u8], gpg_user_id: &str) -> Result<String, String> {
    let mut child = Command::new("gpg")
        .arg("--batch")
        .arg("--yes")
        .arg("--trust-model")
        .arg("always")
        .arg("--encrypt")
        .arg("--recipient")
        .arg(gpg_user_id)
        .arg("--armor")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("無法喚醒 GPG: {}", e))?;

    if let Some(mut stdin) = child.stdin.take() {
        let payload = data.to_vec();
        thread::spawn(move || {
            let _ = stdin.write_all(&payload);
        });
    }

    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    if output.status.success() {
        String::from_utf8(output.stdout).map_err(|e| format!("解編碼錯誤: {}", e))
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

// 🔓 調用本地 GPG 密鑰進行二進位位元組解密（原生支援 .kdbx 等任意二進位實體）
pub fn decrypt_bytes_with_gpg(encrypted_text: &str) -> Result<Vec<u8>, String> {
    let mut child = Command::new("gpg")
        .arg("--batch")
        .arg("--yes")
        .arg("--decrypt")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("無法喚醒 GPG 解密端: {}", e))?;

    if let Some(mut stdin) = child.stdin.take() {
        let payload = encrypted_text.as_bytes().to_vec();
        thread::spawn(move || {
            let _ = stdin.write_all(&payload);
        });
    }

    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(output.stdout) // 🌟 核心：直接傳回原始二進位位元組，不破壞非 UTF-8 結構
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

// 🔓 調用本地 GPG 密鑰進行純文字解密
pub fn decrypt_with_gpg(encrypted_text: &str) -> Result<String, String> {
    let bytes = decrypt_bytes_with_gpg(encrypted_text)?;
    String::from_utf8(bytes).map_err(|e| format!("解編碼錯誤: {}", e))
}