use std::process::Command;
use std::io::Write;

pub fn encrypt_with_gpg(text: &str, gpg_user_id: &str) -> Result<String, String> {
    let mut child = Command::new("gpg")
        .arg("--encrypt")
        .arg("--recipient")
        .arg(gpg_user_id)
        .arg("--armor")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("无法唤醒GPG: {}", e))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
    }

    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    if output.status.success() {
        String::from_utf8(output.stdout).map_err(|e| format!("解编码错误: {}", e))
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

// 🔓 新增：調用本地 GPG 密鑰進行動態物理解密的分支
pub fn decrypt_with_gpg(encrypted_text: &str) -> Result<String, String> {
    let mut child = Command::new("gpg")
        .arg("--decrypt")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("无法唤醒GPG解密端: {}", e))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(encrypted_text.as_bytes()).map_err(|e| e.to_string())?;
    }

    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    if output.status.success() {
        String::from_utf8(output.stdout).map_err(|e| format!("解编码错误: {}", e))
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}