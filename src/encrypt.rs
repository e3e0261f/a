// src/encrypt.rs
// Cyber-NOte 密碼學加密核心模組
// 嚴格規範：僅限 GPG 金鑰體系，禁止任何 SSH 金鑰混用；支援多層巢狀加密與高迭代 S2K 防窮舉加固。

use std::io::Write;
use std::process::{Command, Stdio};
use std::thread;

pub const DEFAULT_S2K_COUNT: u64 = 65011712; // RFC 4880 最大標準迭代輪數 (防 GPU/字典窮舉)

// 🛡️ 安全合規檢查：嚴格拒絕 SSH 金鑰
pub fn validate_gpg_key_not_ssh(key_str: &str) -> Result<(), String> {
    let lower = key_str.trim().to_lowercase();
    if lower.starts_with("ssh-")
        || lower.starts_with("ecdsa-")
        || lower.contains("id_rsa")
        || lower.contains("id_ed25519")
        || lower.contains("id_ecdsa")
        || lower.contains(".ssh/")
        || lower.contains("begin openssh")
        || lower.contains("begin rsa private key")
    {
        return Err("安全合規違規：系統嚴格鎖定 GPG 金鑰，禁止使用 SSH 金鑰！".to_string());
    }
    Ok(())
}

// 🔐 調用本地 GPG 鎖定公鑰進行加密（並發線程寫入，防 64KB 管道死鎖）
pub fn encrypt_with_gpg(data: &[u8], gpg_user_id: &str) -> Result<String, String> {
    validate_gpg_key_not_ssh(gpg_user_id)?;

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
        .map_err(|e| format!("無法調用 GPG 核心: {}", e))?;

    if let Some(mut stdin) = child.stdin.take() {
        let payload = data.to_vec();
        thread::spawn(move || {
            let _ = stdin.write_all(&payload);
        });
    }

    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    if output.status.success() {
        String::from_utf8(output.stdout).map_err(|e| format!("編碼轉換錯誤: {}", e))
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

// 🛡️ 高迭代次數對稱加密（無金鑰 / 密碼模式，防字典與算力窮舉）
// 使用 AES-256、SHA-512、S2K 模式 3 及 65011712 輪強化運算
pub fn encrypt_symmetric_s2k(
    data: &[u8],
    passphrase: &str,
    s2k_count: u64,
) -> Result<String, String> {
    if passphrase.is_empty() {
        return Err("對稱加密密碼不得為空。".to_string());
    }

    let iterations = if s2k_count == 0 { DEFAULT_S2K_COUNT } else { s2k_count };

    let mut child = Command::new("gpg")
        .arg("--batch")
        .arg("--yes")
        .arg("--symmetric")
        .arg("--cipher-algo")
        .arg("AES256")
        .arg("--s2k-mode")
        .arg("3")
        .arg("--s2k-digest-algo")
        .arg("SHA512")
        .arg("--s2k-count")
        .arg(iterations.to_string())
        .arg("--passphrase")
        .arg(passphrase)
        .arg("--armor")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("無法調用 GPG 對稱加密: {}", e))?;

    if let Some(mut stdin) = child.stdin.take() {
        let payload = data.to_vec();
        thread::spawn(move || {
            let _ = stdin.write_all(&payload);
        });
    }

    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    if output.status.success() {
        String::from_utf8(output.stdout).map_err(|e| format!("編碼轉換錯誤: {}", e))
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

// 🔓 調用本地 GPG 進行二進位位元組解密（原生支援公鑰或對稱密鑰解密）
pub fn decrypt_bytes_with_gpg(encrypted_text: &str, passphrase_opt: Option<&str>) -> Result<Vec<u8>, String> {
    let mut cmd = Command::new("gpg");
    cmd.arg("--batch").arg("--yes");

    if let Some(pass) = passphrase_opt {
        cmd.arg("--passphrase").arg(pass);
    }

    cmd.arg("--decrypt")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("無法調用 GPG 解密核心: {}", e))?;

    if let Some(mut stdin) = child.stdin.take() {
        let payload = encrypted_text.as_bytes().to_vec();
        thread::spawn(move || {
            let _ = stdin.write_all(&payload);
        });
    }

    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

// 🔓 調用本地 GPG 進行純文字解密
pub fn decrypt_with_gpg(encrypted_text: &str) -> Result<String, String> {
    let bytes = decrypt_bytes_with_gpg(encrypted_text, None)?;
    String::from_utf8(bytes).map_err(|e| format!("編碼轉換錯誤: {}", e))
}

// 偵測檔案已封裝的 .gpg 層級深度
pub fn calculate_gpg_layer(filename: &str) -> u32 {
    let mut count = 0;
    let mut current = filename;
    while current.ends_with(".gpg") || current.ends_with(".asc") {
        count += 1;
        if current.ends_with(".gpg") {
            current = &current[..current.len() - 4];
        } else {
            current = &current[..current.len() - 4];
        }
    }
    count
}
