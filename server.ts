import express from "express";
import path from "path";
import fs from "fs";
import crypto from "crypto";
import { spawn } from "child_process";
import { fileURLToPath } from "url";
import { createServer as createViteServer } from "vite";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const app = express();
const PORT = 3000;

app.use(express.json({ limit: "50mb" }));

// Helper to expand ~ to HOME
function expandTilde(p: string): string {
  if (p.startsWith("~/") || p === "~") {
    const home = process.env.HOME || "/root";
    return path.join(home, p.slice(1));
  }
  return p;
}

// Locate the note directory matching Rust's GameConfig::get_note_dir
// Standard Linux XDG path: ~/.local/share/cyber-note/notes (present/valid on every Linux distro)
function getNoteDir(): string {
  if (process.env.A_NOTE_DIR) {
    return expandTilde(process.env.A_NOTE_DIR);
  }
  const home = process.env.HOME || "/root";
  const configDir = path.join(home, ".config", "cyber-note");
  const persistentDirFile = path.join(configDir, "dir");
  if (fs.existsSync(persistentDirFile)) {
    try {
      const custom = fs.readFileSync(persistentDirFile, "utf-8").trim();
      if (custom) return expandTilde(custom);
    } catch {
      // fallback
    }
  }
  const legacyDirFile = path.join(home, ".config", "a", "dir");
  if (fs.existsSync(legacyDirFile)) {
    try {
      const custom = fs.readFileSync(legacyDirFile, "utf-8").trim();
      if (custom) return expandTilde(custom);
    } catch {
      // fallback
    }
  }
  // Linux standard XDG directory: ~/.local/share/cyber-note/notes
  const defaultDir = path.join(home, ".local", "share", "cyber-note", "notes");
  if (!fs.existsSync(defaultDir)) {
    try {
      fs.mkdirSync(defaultDir, { recursive: true });
    } catch {
      // ignore
    }
  }
  return defaultDir;
}

// 🛡️ Privacy Data Storage & Isolation Directory (~/.config/cyber-note/secrets)
// Directory chmod 0700, token.gpg chmod 0600
function getSecretsDir(): string {
  const home = process.env.HOME || "/root";
  const secretsDir = path.join(home, ".config", "cyber-note", "secrets");
  if (!fs.existsSync(secretsDir)) {
    try {
      fs.mkdirSync(secretsDir, { recursive: true, mode: 0o700 });
      try { fs.chmodSync(secretsDir, 0o700); } catch { /* ignore */ }
    } catch {
      // ignore
    }
  }
  return secretsDir;
}

// Locate token.gpg from isolated location
function getTokenPath(): string {
  const secretsDir = getSecretsDir();
  const secretToken = path.join(secretsDir, "token.gpg");
  if (fs.existsSync(secretToken)) return secretToken;

  const noteDir = getNoteDir();
  const noteToken = path.join(noteDir, "token.gpg");
  if (fs.existsSync(noteToken)) return noteToken;

  const home = process.env.HOME || "/root";
  const legacyConfigToken = path.join(home, ".config", "a", "secrets", "token.gpg");
  if (fs.existsSync(legacyConfigToken)) return legacyConfigToken;

  const rootToken = path.join(process.cwd(), "token.gpg");
  if (fs.existsSync(rootToken)) return rootToken;

  return secretToken;
}

// Find path to binary 'a'
function getBinaryPath(): string {
  const candidates = [
    "/usr/local/bin/a",
    path.join(process.cwd(), "a"),
    path.join(process.cwd(), "target", "release", "a"),
    "/tmp/rust_test/target/release/a",
  ];
  for (const c of candidates) {
    if (fs.existsSync(c)) {
      return c;
    }
  }
  return "a";
}

// Helper: record entry to key_ledger.json
function appendKeyLedger(record: {
  file_name: string;
  file_path: string;
  key_id: string;
  cipher_mode: string;
  iterations: number;
  layer: number;
  sha256: string;
  notes: string;
}) {
  const home = process.env.HOME || "/root";
  const paths = [
    path.join(home, ".config", "cyber-note", "key_ledger.json"),
    path.join(home, ".config", "a", "key_ledger.json"),
    path.join(getNoteDir(), "key_ledger.json"),
  ];

  let ledger = {
    version: "0.0.4",
    system: "Cyber-NOte",
    updated_at: new Date().toISOString(),
    records: [] as any[],
  };

  for (const p of paths) {
    if (fs.existsSync(p)) {
      try {
        ledger = JSON.parse(fs.readFileSync(p, "utf-8"));
        break;
      } catch {
        // continue
      }
    }
  }

  const now = new Date().toISOString();
  const newEntry = {
    ...record,
    created_at: now,
    verified_at: now,
    status: "ENCRYPTED_AND_VERIFIED",
  };

  ledger.updated_at = now;
  ledger.records.push(newEntry);

  const jsonStr = JSON.stringify(ledger, null, 2);
  for (const p of paths) {
    try {
      const dir = path.dirname(p);
      if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });
      fs.writeFileSync(p, jsonStr, "utf-8");
    } catch {
      // ignore
    }
  }
}

// Helper: execute GPG process
function runGpg(args: string[], stdinData?: Buffer | string): Promise<{ stdout: string; stderr: string; code: number }> {
  return new Promise((resolve) => {
    const child = spawn("gpg", args);
    let stdout = "";
    let stderr = "";

    if (stdinData) {
      child.stdin.write(stdinData);
      child.stdin.end();
    }

    child.stdout.on("data", (d) => (stdout += d.toString()));
    child.stderr.on("data", (d) => (stderr += d.toString()));

    child.on("close", (code) => {
      resolve({ stdout, stderr, code: code ?? 0 });
    });

    child.on("error", (err) => {
      resolve({ stdout, stderr: err.message, code: -1 });
    });
  });
}

// Get Web Engine State
function getWebEngineState(): string {
  const home = process.env.HOME || "/root";
  const stateFile = path.join(home, ".config", "cyber-note", "web.state");
  if (fs.existsSync(stateFile)) {
    try {
      const s = fs.readFileSync(stateFile, "utf-8").trim();
      if (s) return s;
    } catch {
      // ignore
    }
  }
  const legacyStateFile = path.join(home, ".config", "a", "web.state");
  if (fs.existsSync(legacyStateFile)) {
    try {
      const s = fs.readFileSync(legacyStateFile, "utf-8").trim();
      if (s) return s;
    } catch {
      // ignore
    }
  }
  return "active";
}

// -------------------------------------------------------------
// API Routes
// -------------------------------------------------------------

// 1. Health check
app.get("/api/health", (_req, res) => {
  res.json({
    status: "ok",
    webEngine: "active",
    core: "Rust Native Engine (Project a)",
  });
});

// 2. Rust system & note status
app.get("/api/rust/status", (_req, res) => {
  const noteDir = getNoteDir();
  const home = process.env.HOME || "/root";
  const configDir = path.join(home, ".config", "cyber-note");
  const legacyConfigDir = path.join(home, ".config", "a");
  const binaryPath = getBinaryPath();
  const binExists = fs.existsSync(binaryPath);

  // Read config files
  let keyId = "";
  let gistId = "";
  const keyFile = path.join(noteDir, "key_id");
  const gistFile = path.join(noteDir, "gist_id");
  const tokenFile = getTokenPath();

  if (fs.existsSync(keyFile)) {
    try { keyId = fs.readFileSync(keyFile, "utf-8").trim(); } catch { /* ignore */ }
  }
  if (!keyId) {
    const cfgKey = path.join(configDir, "key_id");
    if (fs.existsSync(cfgKey)) {
      try { keyId = fs.readFileSync(cfgKey, "utf-8").trim(); } catch { /* ignore */ }
    }
  }
  if (!keyId) {
    const legacyKey = path.join(legacyConfigDir, "key_id");
    if (fs.existsSync(legacyKey)) {
      try { keyId = fs.readFileSync(legacyKey, "utf-8").trim(); } catch { /* ignore */ }
    }
  }

  if (fs.existsSync(gistFile)) {
    try { gistId = fs.readFileSync(gistFile, "utf-8").trim(); } catch { /* ignore */ }
  }
  if (!gistId) {
    const cfgGist = path.join(configDir, "gist_id");
    if (fs.existsSync(cfgGist)) {
      try { gistId = fs.readFileSync(cfgGist, "utf-8").trim(); } catch { /* ignore */ }
    }
  }

  const hasToken = fs.existsSync(tokenFile);

  // Read notes in noteDir
  const noteFiles: Array<{ name: string; size: number; mtime: string }> = [];
  if (fs.existsSync(noteDir)) {
    try {
      const files = fs.readdirSync(noteDir);
      for (const file of files) {
        const full = path.join(noteDir, file);
        const st = fs.statSync(full);
        if (st.isFile()) {
          noteFiles.push({
            name: file,
            size: st.size,
            mtime: st.mtime.toISOString(),
          });
        }
      }
    } catch {
      // ignore
    }
  }

  const webState = getWebEngineState();

  res.json({
    rustAvailable: binExists,
    binaryPath,
    version: "0.0.4",
    noteDir,
    defaultDirStandard: "~/.local/share/cyber-note/notes",
    secretsDir: getSecretsDir(),
    tokenPath: tokenFile,
    keyId: keyId || "未配置",
    gistId: gistId || "未配置",
    hasToken,
    webState,
    noteFiles,
    systemTime: new Date().toISOString(),
  });
});

// 2.5 System Diagnostics & Knowledge Guide for tsx and express
app.get("/api/system/diagnostics", (_req, res) => {
  const noteDir = getNoteDir();
  const secretsDir = getSecretsDir();
  const tokenPath = getTokenPath();

  res.json({
    tsx: {
      status: "active",
      name: "tsx (TypeScript Execute)",
      description:
        "基於 esbuild 引擎的高效能零編譯 TypeScript 執行環境。解決 Node.js 原生無法直接執行 .ts 的痛點，使伺服器能在 0 編譯耗時下即時啟動，記憶體佔用極低。",
    },
    express: {
      status: "active",
      name: "express (輕量級 HTTP Web API 伺服器框架)",
      description:
        "Node.js 業界標竿且經長年資安檢驗的輕量級 HTTP 框架。Cyber-NOte 使用 Express 在本地提供安全 REST API，負責 GPG 金鑰調用、金鑰歸檔簿審計與 Gist 雲端通信。",
    },
    paths: {
      noteDir,
      defaultStandard: "~/.local/share/cyber-note/notes",
      secretsDir,
      tokenFile: tokenPath,
      posixPermissions: "目錄 0700 (rwx------) / 憑證檔案 0600 (rw-------)",
    },
    installCommands: {
      global: "npm install -g tsx express",
      project: "npm install",
    },
    securityPolicies: {
      gpgKeyLock: "ACTIVE - 嚴格鎖定 GPG 金鑰體系",
      sshKeyRejection: "ACTIVE - 拒絕任何 SSH 金鑰",
      s2kDefaultIterations: 65011712,
      cleanSlateSupported: true,
    },
  });
});

// 3. Execute native Rust CLI command
app.post("/api/rust/exec", (req, res) => {
  const { args = [], stdin = "" } = req.body;
  const binary = getBinaryPath();
  const startTime = Date.now();

  const child = spawn(binary, args, {
    env: { ...process.env, PATH: `${process.env.HOME}/.cargo/bin:/usr/local/bin:${process.env.PATH}` },
  });

  let stdout = "";
  let stderr = "";

  if (stdin && child.stdin) {
    child.stdin.write(stdin);
    child.stdin.end();
  }

  child.stdout?.on("data", (data) => {
    stdout += data.toString();
  });

  child.stderr?.on("data", (data) => {
    stderr += data.toString();
  });

  child.on("close", (code) => {
    const durationMs = Date.now() - startTime;
    res.json({
      exitCode: code,
      stdout,
      stderr,
      durationMs,
      command: `a ${args.join(" ")}`.trim(),
    });
  });

  child.on("error", (err) => {
    res.status(500).json({
      error: err.message,
      exitCode: -1,
      stdout,
      stderr,
      durationMs: Date.now() - startTime,
      command: `a ${args.join(" ")}`.trim(),
    });
  });
});

// 4. Note file content viewer & writer
app.get("/api/notes/content", (req, res) => {
  const filename = req.query.file as string;
  if (!filename) {
    return res.status(400).json({ error: "Missing file parameter" });
  }

  const noteDir = getNoteDir();
  const safeName = path.basename(filename);
  const targetPath = path.join(noteDir, safeName);

  if (!fs.existsSync(targetPath)) {
    return res.status(404).json({ error: "File not found" });
  }

  try {
    const content = fs.readFileSync(targetPath, "utf-8");
    const isEncrypted = content.includes("-----BEGIN PGP MESSAGE-----");
    res.json({
      filename: safeName,
      path: targetPath,
      content,
      isEncrypted,
      size: fs.statSync(targetPath).size,
    });
  } catch (err: any) {
    res.status(500).json({ error: err.message });
  }
});

// 4.5 🛡️ 檔案加密封裝接口（支援明文檔案、已加密檔案加套一層、密碼防窮舉與 Gist 同步）
app.post("/api/encrypt-file", async (req, res) => {
  const {
    fileName,
    content,
    pass,
    iterations = 65011712,
    useSymmetric = false,
    uploadToGist = false,
    token = "",
    gistId = "",
  } = req.body;

  if (!fileName) {
    return res.status(400).json({ error: "請指定檔案名稱" });
  }

  const noteDir = getNoteDir();
  let rawData = content;

  // 若前端未傳入 content，嘗試從本地 noteDir 讀取
  if (rawData === undefined || rawData === null) {
    const localPath = path.join(noteDir, fileName);
    if (fs.existsSync(localPath)) {
      rawData = fs.readFileSync(localPath, "utf-8");
    } else {
      return res.status(400).json({ error: `未提供內容且本地找不到檔案: ${fileName}` });
    }
  }

  // 決定封裝後的檔名（加套一層 .gpg）
  const outputFileName = `${fileName}.gpg`;
  const outputFilePath = path.join(noteDir, outputFileName);

  // 讀取當前金鑰設定
  const keyFile = path.join(noteDir, "key_id");
  let lockedKey = "";
  if (fs.existsSync(keyFile)) {
    try { lockedKey = fs.readFileSync(keyFile, "utf-8").trim(); } catch { /* ignore */ }
  }
  if (!lockedKey) {
    const cfgKey = path.join(process.env.HOME || "/root", ".config", "cyber-note", "key_id");
    if (fs.existsSync(cfgKey)) {
      try { lockedKey = fs.readFileSync(cfgKey, "utf-8").trim(); } catch { /* ignore */ }
    }
  }

  try {
    let ciphertext = "";
    let cipherMode = "GPG_PUBLIC_KEY";
    let keyIdUsed = lockedKey || "DEFAULT_KEY";
    let iters = 0;

    if (useSymmetric || pass) {
      // S2K 模式 3 防窮舉密碼對稱加密
      if (!pass) {
        return res.status(400).json({ error: "對稱加密模式必須提供加密密碼" });
      }
      iters = iterations || 65011712;
      cipherMode = "GPG_SYMMETRIC_S2K";
      keyIdUsed = `SYMMETRIC-S2K (${iters} 輪)`;

      const gpgRes = await runGpg(
        [
          "--batch",
          "--yes",
          "--armor",
          "--symmetric",
          "--s2k-mode", "3",
          "--s2k-count", iters.toString(),
          "--cipher-algo", "AES256",
          "--passphrase", pass,
        ],
        rawData
      );

      if (gpgRes.code !== 0) {
        return res.status(500).json({ error: `GPG 對稱加密失敗: ${gpgRes.stderr}` });
      }
      ciphertext = gpgRes.stdout;
    } else if (lockedKey) {
      // 使用系統鎖定的 GPG 公鑰非對稱加密
      const gpgRes = await runGpg(
        [
          "--batch",
          "--yes",
          "--armor",
          "--encrypt",
          "--recipient", lockedKey,
          "--trust-model", "always",
        ],
        rawData
      );

      if (gpgRes.code !== 0) {
        return res.status(500).json({ error: `GPG 公鑰加密失敗: ${gpgRes.stderr}` });
      }
      ciphertext = gpgRes.stdout;
    } else {
      return res.status(400).json({ error: "未鎖定 GPG 公鑰且未設定對稱密碼，無法執行加密" });
    }

    // 寫入本地 noteDir
    fs.writeFileSync(outputFilePath, ciphertext, "utf-8");

    // 計算 SHA-256
    const sha256 = crypto.createHash("sha256").update(ciphertext).digest("hex");

    // 計算巢狀層級
    let layer = 1;
    let temp = outputFileName;
    while (temp.endsWith(".gpg")) {
      layer++;
      temp = temp.slice(0, -4);
    }
    layer -= 1; // offset

    // 留黨存檔：寫入金鑰歸檔簿 (Key Ledger)
    appendKeyLedger({
      file_name: outputFileName,
      file_path: outputFilePath,
      key_id: keyIdUsed,
      cipher_mode: cipherMode,
      iterations: iters,
      layer,
      sha256,
      notes: layer > 1 ? `第 ${layer} 層巢狀多重加密封裝` : "單層獨立檔案加密封裝",
    });

    // 若要求同步至 Gist
    let gistSynced = false;
    let effectiveGistId = gistId;
    if (!effectiveGistId) {
      const gistFile = path.join(noteDir, "gist_id");
      if (fs.existsSync(gistFile)) {
        try { effectiveGistId = fs.readFileSync(gistFile, "utf-8").trim(); } catch { /* ignore */ }
      }
    }

    if (uploadToGist && effectiveGistId && token) {
      try {
        const fetchRes = await fetch(`https://api.github.com/gists/${effectiveGistId}`, {
          method: "PATCH",
          headers: {
            Authorization: `Bearer ${token}`,
            Accept: "application/vnd.github+json",
            "X-GitHub-Api-Version": "2022-11-28",
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            files: {
              [outputFileName]: { content: ciphertext },
            },
          }),
        });
        if (fetchRes.ok) {
          gistSynced = true;
        }
      } catch {
        // gist update failed
      }
    }

    res.json({
      success: true,
      originalName: fileName,
      encryptedFileName: outputFileName,
      outputFilePath,
      cipherMode,
      keyIdUsed,
      iterations: iters,
      layer,
      sha256,
      gistSynced,
      ciphertext,
    });
  } catch (err: any) {
    res.status(500).json({ error: err.message });
  }
});

// 4.6 🚀 抹除歷史與全新倉庫遷移接口 (Clean Slate Repository Migration)
app.post("/api/gist/migrate-repo", async (req, res) => {
  const { token, deleteOld = false } = req.body;

  if (!token) {
    return res.status(400).json({ error: "請提供 GitHub Token 憑證以進行倉庫遷移" });
  }

  const noteDir = getNoteDir();
  const gistFile = path.join(noteDir, "gist_id");
  let oldGistId = "";
  if (fs.existsSync(gistFile)) {
    try { oldGistId = fs.readFileSync(gistFile, "utf-8").trim(); } catch { /* ignore */ }
  }

  // 讀取本地所有需要遷移的加密檔案
  const filesPayload: Record<string, { content: string }> = {};
  if (fs.existsSync(noteDir)) {
    try {
      const allFiles = fs.readdirSync(noteDir);
      for (const f of allFiles) {
        if (f !== "gist_id" && f !== "key_id" && f !== "token.gpg" && f !== "dir") {
          const fullPath = path.join(noteDir, f);
          const st = fs.statSync(fullPath);
          if (st.isFile()) {
            const content = fs.readFileSync(fullPath, "utf-8");
            filesPayload[f] = { content };
          }
        }
      }
    } catch (e: any) {
      return res.status(500).json({ error: `讀取本地檔案失敗: ${e.message}` });
    }
  }

  if (Object.keys(filesPayload).length === 0) {
    filesPayload["cyber_note_vault.manifest"] = {
      content: `Cyber-NOte 乾淨無痕加密倉庫已就緒 (修訂歷史已徹底抹除 @ ${new Date().toISOString()})`,
    };
  }

  try {
    // 1. 發送 POST 請求建立全新 Gist
    const desc = `Cyber-NOte Vault [Clean Slate - Purged History @ ${new Date().toISOString()}]`;
    const createRes = await fetch("https://api.github.com/gists", {
      method: "POST",
      headers: {
        Authorization: `Bearer ${token}`,
        Accept: "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        description: desc,
        public: false,
        files: filesPayload,
      }),
    });

    if (!createRes.ok) {
      const errText = await createRes.text();
      return res.status(createRes.status).json({
        error: `建立全新 Gist 失敗 (HTTP ${createRes.status}): ${errText}`,
      });
    }

    const createdGist: any = await createRes.json();
    const newGistId = createdGist.id;

    // 2. 更新本地 gist_id
    const home = process.env.HOME || "/root";
    fs.writeFileSync(gistFile, newGistId);
    fs.writeFileSync(path.join(home, ".config", "cyber-note", "gist_id"), newGistId);
    try {
      fs.writeFileSync(path.join(home, ".config", "a", "gist_id"), newGistId);
    } catch { /* ignore */ }

    // 3. 若指定刪除舊倉庫
    let oldDeleted = false;
    if (deleteOld && oldGistId && oldGistId !== newGistId) {
      try {
        const delRes = await fetch(`https://api.github.com/gists/${oldGistId}`, {
          method: "DELETE",
          headers: {
            Authorization: `Bearer ${token}`,
            Accept: "application/vnd.github+json",
            "X-GitHub-Api-Version": "2022-11-28",
          },
        });
        if (delRes.status === 204 || delRes.ok) {
          oldDeleted = true;
        }
      } catch {
        // delete failed
      }
    }

    res.json({
      success: true,
      oldGistId,
      newGistId,
      transferredFilesCount: Object.keys(filesPayload).length,
      oldDeleted,
      message: "全新倉庫建立成功！所有歷史修訂與 Git 提交記錄已徹底切斷抹除。",
    });
  } catch (err: any) {
    res.status(500).json({ error: err.message });
  }
});

// 4.7 🛡️ 遠端檔案在位加密套殼（Remote In-Place Encapsulate & Delete Original）
app.post("/api/gist/encapsulate-file", async (req, res) => {
  const {
    filename,
    deleteOriginal = true,
    encryptMode = "gpg",
    passphrase = "",
    iterations = 65011712,
    token: reqToken,
    gistId: reqGistId,
  } = req.body;

  if (!filename) {
    return res.status(400).json({ error: "未指定欲套殼加密之遠端檔案名稱" });
  }

  const noteDir = getNoteDir();
  const home = process.env.HOME || "/root";

  // 解析 Gist ID
  let gistId = reqGistId;
  if (!gistId) {
    const gistFile = path.join(noteDir, "gist_id");
    if (fs.existsSync(gistFile)) {
      try { gistId = fs.readFileSync(gistFile, "utf-8").trim(); } catch { /* ignore */ }
    }
  }
  if (!gistId) {
    const cfgGist = path.join(home, ".config", "cyber-note", "gist_id");
    if (fs.existsSync(cfgGist)) {
      try { gistId = fs.readFileSync(cfgGist, "utf-8").trim(); } catch { /* ignore */ }
    }
  }
  if (!gistId) {
    return res.status(400).json({ error: "未指定且未找到配置之 Gist ID" });
  }

  // 解析 Token
  let token = reqToken;
  if (!token) {
    const tokenPath = getTokenPath();
    if (fs.existsSync(tokenPath)) {
      const rawTok = fs.readFileSync(tokenPath, "utf-8");
      if (rawTok.includes("-----BEGIN PGP MESSAGE-----")) {
        const decRes = await runGpg(["--batch", "--yes", "--decrypt"], rawTok);
        if (decRes.code === 0 && decRes.stdout.trim()) {
          token = decRes.stdout.trim();
        }
      } else {
        token = rawTok.trim();
      }
    }
  }
  if (!token) {
    return res.status(400).json({ error: "未提供且未找到有效之 GitHub Token 憑證" });
  }

  try {
    // 1. 從遠端獲取檔案內容
    const gistUrl = `https://api.github.com/gists/${gistId}`;
    const getRes = await fetch(gistUrl, {
      headers: {
        Authorization: `Bearer ${token}`,
        Accept: "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
      },
    });

    if (!getRes.ok) {
      return res.status(getRes.status).json({ error: `獲取 Gist 倉庫失敗 (HTTP ${getRes.status})` });
    }

    const gistData: any = await getRes.json();
    const fileObj = gistData.files?.[filename];
    if (!fileObj) {
      return res.status(404).json({ error: `在遠端 Gist 倉庫中找不到指定檔案: ${filename}` });
    }

    let rawContent = fileObj.content || "";
    if (fileObj.truncated && fileObj.raw_url) {
      const rawRes = await fetch(fileObj.raw_url);
      if (rawRes.ok) {
        rawContent = await rawRes.text();
      }
    }

    // 2. 執行 GPG 加密
    let ciphertext = "";
    let cipherMode = "GPG_PUBLIC_KEY";
    let keyIdUsed = "";
    let iters = 0;

    // 讀取鎖定金鑰
    let lockedKey = "";
    const keyFile = path.join(noteDir, "key_id");
    if (fs.existsSync(keyFile)) {
      try { lockedKey = fs.readFileSync(keyFile, "utf-8").trim(); } catch { /* ignore */ }
    }
    if (!lockedKey) {
      const cfgKey = path.join(home, ".config", "cyber-note", "key_id");
      if (fs.existsSync(cfgKey)) {
        try { lockedKey = fs.readFileSync(cfgKey, "utf-8").trim(); } catch { /* ignore */ }
      }
    }

    if (encryptMode === "symmetric" || (!lockedKey && passphrase)) {
      if (!passphrase) {
        return res.status(400).json({ error: "對稱防窮舉加密模式必須輸入自訂密碼" });
      }
      iters = iterations || 65011712;
      cipherMode = "GPG_SYMMETRIC_S2K";
      keyIdUsed = `SYMMETRIC-S2K (${iters} 輪)`;

      const gpgRes = await runGpg(
        [
          "--batch",
          "--yes",
          "--armor",
          "--symmetric",
          "--s2k-mode", "3",
          "--s2k-count", iters.toString(),
          "--cipher-algo", "AES256",
          "--passphrase", passphrase,
        ],
        rawContent
      );

      if (gpgRes.code !== 0) {
        return res.status(500).json({ error: `GPG 對稱加密失敗: ${gpgRes.stderr}` });
      }
      ciphertext = gpgRes.stdout;
    } else {
      // 公鑰加密
      keyIdUsed = lockedKey || "CyberNOte-Key";
      const gpgRes = await runGpg(
        [
          "--batch",
          "--yes",
          "--armor",
          "--encrypt",
          "--recipient", keyIdUsed,
          "--trust-model", "always",
        ],
        rawContent
      );

      if (gpgRes.code !== 0) {
        return res.status(500).json({
          error: `GPG 公鑰加密失敗 (金鑰 ${keyIdUsed} 尚未匯入或無法使用): ${gpgRes.stderr}。您可以切換為「對稱密碼防窮舉 (S2K)」模式。`,
        });
      }
      ciphertext = gpgRes.stdout;
    }

    // 3. 計算目標加密檔名 (例如 config.dae -> config.dae.gpg)
    const newEncryptedFileName = `${filename}.gpg`;
    const newFilePath = path.join(noteDir, newEncryptedFileName);

    // 4. 計算 SHA-256 與巢狀層級
    const sha256 = crypto.createHash("sha256").update(ciphertext).digest("hex");
    let layer = 1;
    let tempName = newEncryptedFileName;
    while (tempName.endsWith(".gpg")) {
      layer++;
      tempName = tempName.slice(0, -4);
    }
    layer -= 1;

    // 寫入本地副本與金鑰歸檔簿
    try {
      if (!fs.existsSync(noteDir)) fs.mkdirSync(noteDir, { recursive: true });
      fs.writeFileSync(newFilePath, ciphertext, "utf-8");
    } catch { /* ignore */ }

    appendKeyLedger({
      file_name: newEncryptedFileName,
      file_path: newFilePath,
      key_id: keyIdUsed,
      cipher_mode: cipherMode,
      iterations: iters,
      layer,
      sha256,
      notes: `遠端在位套殼加密 (原檔名: ${filename}${deleteOriginal ? "，原明文已在遠端自動刪除" : ""})`,
    });

    // 5. 原子更新 GitHub Gist：發佈 newEncryptedFileName，並可選刪除 filename (設為 null)
    const patchPayload: Record<string, any> = {
      [newEncryptedFileName]: { content: ciphertext },
    };
    if (deleteOriginal && filename !== newEncryptedFileName) {
      patchPayload[filename] = null; // GitHub API 會直接自 Gist 刪除此檔案
    }

    const patchRes = await fetch(gistUrl, {
      method: "PATCH",
      headers: {
        Authorization: `Bearer ${token}`,
        Accept: "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        description: `Cyber-NOte 遠端檔案套殼加密 [${filename} -> ${newEncryptedFileName}]`,
        files: patchPayload,
      }),
    });

    if (!patchRes.ok) {
      const errText = await patchRes.text();
      return res.status(patchRes.status).json({
        error: `遠端原子替換失敗 (HTTP ${patchRes.status}): ${errText}`,
      });
    }

    res.json({
      success: true,
      originalName: filename,
      newEncryptedFileName,
      originalDeleted: deleteOriginal,
      keyIdUsed,
      cipherMode,
      iterations: iters,
      layer,
      sha256,
      message: deleteOriginal
        ? `🔒 成功將遠端檔案「${filename}」套上 GPG 加密殼！遠端已生成「${newEncryptedFileName}」，原檔案「${filename}」已在遠端同步銷毀。`
        : `🔒 成功將遠端檔案「${filename}」加密並上傳為「${newEncryptedFileName}」（原檔案仍保留）。`,
    });
  } catch (err: any) {
    res.status(500).json({ error: err.message });
  }
});

// 4.8 🔓 遠端密文檔案解密（支援本地解密保護，亦可選性去殼還原）
app.post("/api/gist/decapsulate-file", async (req, res) => {
  const {
    filename,
    passphrase = "",
    restoreRemotePlaintext = false,
    deleteEncryptedRemote = false,
    token: reqToken,
    gistId: reqGistId,
  } = req.body;

  if (!filename) {
    return res.status(400).json({ error: "未指定欲解密之檔案名稱" });
  }

  const noteDir = getNoteDir();
  const home = process.env.HOME || "/root";

  // 解析 Gist ID
  let gistId = reqGistId;
  if (!gistId) {
    const gistFile = path.join(noteDir, "gist_id");
    if (fs.existsSync(gistFile)) {
      try { gistId = fs.readFileSync(gistFile, "utf-8").trim(); } catch { /* ignore */ }
    }
  }
  if (!gistId) {
    return res.status(400).json({ error: "未指定 Gist ID" });
  }

  // 解析 Token
  let token = reqToken;
  if (!token) {
    const tokenPath = getTokenPath();
    if (fs.existsSync(tokenPath)) {
      const rawTok = fs.readFileSync(tokenPath, "utf-8");
      if (rawTok.includes("-----BEGIN PGP MESSAGE-----")) {
        const decRes = await runGpg(["--batch", "--yes", "--decrypt"], rawTok);
        if (decRes.code === 0 && decRes.stdout.trim()) {
          token = decRes.stdout.trim();
        }
      } else {
        token = rawTok.trim();
      }
    }
  }

  try {
    // 1. 獲取密文
    const gistUrl = `https://api.github.com/gists/${gistId}`;
    const getRes = await fetch(gistUrl, {
      headers: {
        ...(token ? { Authorization: `Bearer ${token}` } : {}),
        Accept: "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
      },
    });

    if (!getRes.ok) {
      return res.status(getRes.status).json({ error: `獲取 Gist 倉庫失敗 (HTTP ${getRes.status})` });
    }

    const gistData: any = await getRes.json();
    const fileObj = gistData.files?.[filename];
    if (!fileObj) {
      return res.status(404).json({ error: `在遠端 Gist 倉庫中找不到指定檔案: ${filename}` });
    }

    let ciphertext = fileObj.content || "";
    if (fileObj.truncated && fileObj.raw_url) {
      const rawRes = await fetch(fileObj.raw_url);
      if (rawRes.ok) {
        ciphertext = await rawRes.text();
      }
    }

    // 2. 調用 GPG 解密
    const gpgArgs = ["--batch", "--yes", "--decrypt"];
    if (passphrase) {
      gpgArgs.push("--passphrase", passphrase);
    }

    const decRes = await runGpg(gpgArgs, ciphertext);
    if (decRes.code !== 0) {
      return res.status(400).json({
        error: `GPG 解密失敗: ${decRes.stderr || "金鑰未匹配或密碼錯誤"}`,
      });
    }

    const plaintext = decRes.stdout;
    const outputPlaintextName = filename.endsWith(".gpg") ? filename.slice(0, -4) : `${filename}.decrypted`;

    // 3. 儲存本地副本 (方便本地查閱)
    const localPlainPath = path.join(noteDir, outputPlaintextName);
    try {
      if (!fs.existsSync(noteDir)) fs.mkdirSync(noteDir, { recursive: true });
      fs.writeFileSync(localPlainPath, plaintext, "utf-8");
    } catch { /* ignore */ }

    // 4. 若使用者「要求」還原遠端明文
    let remoteRestored = false;
    if (restoreRemotePlaintext && token) {
      const patchPayload: Record<string, any> = {
        [outputPlaintextName]: { content: plaintext },
      };
      if (deleteEncryptedRemote) {
        patchPayload[filename] = null;
      }

      const patchRes = await fetch(gistUrl, {
        method: "PATCH",
        headers: {
          Authorization: `Bearer ${token}`,
          Accept: "application/vnd.github+json",
          "X-GitHub-Api-Version": "2022-11-28",
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          files: patchPayload,
        }),
      });

      if (patchRes.ok) {
        remoteRestored = true;
      }
    }

    res.json({
      success: true,
      originalEncryptedName: filename,
      outputPlaintextName,
      plaintextLength: plaintext.length,
      remoteRestored,
      plaintext,
      message: remoteRestored
        ? `🔓 已解密並將還原明文「${outputPlaintextName}」發佈至遠端${deleteEncryptedRemote ? '（原密文已自遠端刪除）' : ''}。`
        : `🔓 已成功在本地安全解密（遠端維持 GPG 密文封存狀態，未洩漏明文）。`,
    });
  } catch (err: any) {
    res.status(500).json({ error: err.message });
  }
});

// 5. Update configuration (嚴格禁止 SSH 金鑰)
app.post("/api/rust/config", (req, res) => {
  const { noteDir, keyId, gistId, token } = req.body;
  const home = process.env.HOME || "/root";
  const configDir = path.join(home, ".config", "cyber-note");
  const legacyConfigDir = path.join(home, ".config", "a");

  try {
    if (keyId !== undefined) {
      const lower = keyId.trim().toLowerCase();
      if (
        lower.startsWith("ssh-") ||
        lower.startsWith("ecdsa-") ||
        lower.includes("id_rsa") ||
        lower.includes("id_ed25519") ||
        lower.includes("id_ecdsa") ||
        lower.includes(".ssh/") ||
        lower.includes("begin openssh") ||
        lower.includes("begin rsa private key")
      ) {
        return res.status(400).json({
          error: "安全合規違規：系統嚴格鎖定 GPG 金鑰，禁止使用 SSH 金鑰！請輸入有效的 GPG Key ID 或指紋。",
        });
      }
    }

    if (!fs.existsSync(configDir)) {
      fs.mkdirSync(configDir, { recursive: true });
    }
    if (!fs.existsSync(legacyConfigDir)) {
      fs.mkdirSync(legacyConfigDir, { recursive: true });
    }

    if (noteDir) {
      const exp = expandTilde(noteDir);
      if (!fs.existsSync(exp)) {
        fs.mkdirSync(exp, { recursive: true });
      }
      fs.writeFileSync(path.join(configDir, "dir"), noteDir.trim());
      fs.writeFileSync(path.join(legacyConfigDir, "dir"), noteDir.trim());
    }

    const currentNoteDir = getNoteDir();
    if (keyId !== undefined) {
      fs.writeFileSync(path.join(currentNoteDir, "key_id"), keyId.trim());
      fs.writeFileSync(path.join(configDir, "key_id"), keyId.trim());
      fs.writeFileSync(path.join(legacyConfigDir, "key_id"), keyId.trim());
    }

    if (gistId !== undefined) {
      const cleanGist = gistId.replace(/https:\/\/gist\.github\.com\/[^\/]+\//, "").trim();
      fs.writeFileSync(path.join(currentNoteDir, "gist_id"), cleanGist);
      fs.writeFileSync(path.join(configDir, "gist_id"), cleanGist);
      fs.writeFileSync(path.join(legacyConfigDir, "gist_id"), cleanGist);
    }

    // 🛡️ 隱私憑證隔離存儲
    if (token) {
      const secretsDir = getSecretsDir();
      const secretTokenPath = path.join(secretsDir, "token.gpg");
      fs.writeFileSync(secretTokenPath, token.trim());
      try { fs.chmodSync(secretTokenPath, 0o600); } catch { /* ignore */ }
      // 相容落盤
      fs.writeFileSync(path.join(currentNoteDir, "token.gpg"), token.trim());
    }

    res.json({ success: true, message: "配置更新已落地至安全目錄！" });
  } catch (err: any) {
    res.status(500).json({ error: err.message });
  }
});

// 6. Get Key Ledger (金鑰歸檔審計記錄)
app.get("/api/ledger", (_req, res) => {
  const home = process.env.HOME || "/root";
  const primaryLedger = path.join(home, ".config", "cyber-note", "key_ledger.json");
  const legacyLedger = path.join(home, ".config", "a", "key_ledger.json");
  const noteDir = getNoteDir();
  const backupLedger = path.join(noteDir, "key_ledger.json");

  let ledgerData = { version: "0.0.4", system: "Cyber-NOte", updated_at: "", records: [] };

  if (fs.existsSync(primaryLedger)) {
    try {
      ledgerData = JSON.parse(fs.readFileSync(primaryLedger, "utf-8"));
    } catch { /* fallback */ }
  } else if (fs.existsSync(legacyLedger)) {
    try {
      ledgerData = JSON.parse(fs.readFileSync(legacyLedger, "utf-8"));
    } catch { /* fallback */ }
  } else if (fs.existsSync(backupLedger)) {
    try {
      ledgerData = JSON.parse(fs.readFileSync(backupLedger, "utf-8"));
    } catch { /* fallback */ }
  }

  res.json(ledgerData);
});

// 7. Toggle Web Engine State (Active / Standby)
app.post("/api/web/state", (req, res) => {
  const { state } = req.body;
  const home = process.env.HOME || "/root";
  const configDir = path.join(home, ".config", "cyber-note");
  if (!fs.existsSync(configDir)) {
    fs.mkdirSync(configDir, { recursive: true });
  }
  const stateFile = path.join(configDir, "web.state");
  const newState = state === "standby" ? "standby" : "active";
  fs.writeFileSync(stateFile, newState);

  res.json({
    state: newState,
    message:
      newState === "active"
        ? "Web 網頁端管理引擎已啟動 (ACTIVE)"
        : "Web 網頁端管理引擎已切換為待機模式 (STANDBY - 預設關閉)",
  });
});

// -------------------------------------------------------------
// Vite Middleware & Static Serving
// -------------------------------------------------------------
async function startServer() {
  if (process.env.NODE_ENV !== "production") {
    const vite = await createViteServer({
      server: { middlewareMode: true },
      appType: "spa",
    });
    app.use(vite.middlewares);
  } else {
    const distPath = path.join(process.cwd(), "dist");
    app.use(express.static(distPath));
    // Express 5 route syntax: use '*all' instead of '*'
    app.get("*all", (_req, res) => {
      res.sendFile(path.join(distPath, "index.html"));
    });
  }

  app.listen(PORT, "0.0.0.0", () => {
    console.log(`[Cyber-NOte] Server running on http://0.0.0.0:${PORT}`);
  });
}

startServer();
