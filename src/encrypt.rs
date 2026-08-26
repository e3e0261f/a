use std::process::Command;
use std::io::Write;

// 🔐 函數體：調用本地 GPG 密鑰進行動態物理加密的公式
pub fn encrypt_with_gpg(text: &str, gpg_user_id: &str) -> Result<String, String> {
    // ➔ 喚醒系統底層的 gpg 執行單元
    let mut child = Command::new("gpg")
        .arg("--encrypt")
        .arg("--recipient")
        .arg(gpg_user_id)
        .arg("--armor") // 👈 核心：吐出乾淨的 ASCII 文本亂碼，方便 JSON 傳輸
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("无法唤醒GPG: {}", e))?;

    // ➔ 把純文本靈感，通過管道塞進 GPG 的碎纸機裡
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
    }

    // ➔ 撈出加密後的终极亂碼包裹
    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    if output.status.success() {
        String::from_utf8(output.stdout).map_err(|e| format!("解编码错误: {}", e))
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}
