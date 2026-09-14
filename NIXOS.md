# ❄️ Cyber-NOte · NixOS 100% 深度適配指南

本專案提供對 **NixOS** 與 **Nix Flakes** 體系的 **100% 原生支援**。

透過 `makeWrapper` 運行時環境包裹、完整的 `nixosModules` 系統級守護服務、`homeManagerModules` 使用者級配置，以及自動化 GnuPG Agent 整合，徹底解決 NixOS 純淨隔離環境下 `gpg` 指令缺失、`pinentry` 密碼彈窗阻塞、以及不可寫入 `/usr/bin` 的所有痛點。

---

## ⚡ 1. 快速開始 (免安裝即時體驗)

在任何已啟用 Nix Flakes 的系統（NixOS, macOS, Linux）上直接執行：

```bash
# 🎯 直接運行 CLI
nix run github:e3e0261f/a -- --help

# 📝 直接記錄一筆加密筆記
nix run github:e3e0261f/a -- "這是一條受 GPG 保護的隱密訊息"

# 🌐 直接啟動網頁端管理後台
nix run github:e3e0261f/a#web
```

---

## 🛠️ 2. 純淨隔離開發環境 (`nix develop`)

進入預先配置好 Rust 2024 工具鏈、OpenSSL、GnuPG、Node.js 22 與開發依賴的隔離 Shell：

```bash
nix develop
```

在 Shell 內部：
- 自動掛載 `RUST_SRC_PATH` 與 `PKG_CONFIG_PATH`
- `cargo build --release` 直接編譯
- `cargo run -- --web` 即刻啟動網頁後台

---

## 🛡️ 3. NixOS 系統級模組配置 (`configuration.nix`)

將 Cyber-NOte 作為系統服務整合至 NixOS，提供全域指令與可選的 Systemd 背景常駐 Web 管理引擎：

### (1) 在您的系統 `flake.nix` 中引入輸入：
```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    cyber-note.url = "github:e3e0261f/a";
  };

  outputs = { self, nixpkgs, cyber-note, ... }: {
    nixosConfigurations.my-nixos-host = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        # 引入 Cyber-NOte NixOS 模組
        cyber-note.nixosModules.default

        # 您的系統設定
        ./configuration.nix
      ];
    };
  };
}
```

### (2) 在 `/etc/nixos/configuration.nix` 啟用服務：
```nix
{ config, pkgs, ... }:

{
  # 🚀 啟用 Cyber-NOte 系統級服務
  services.cyber-note = {
    enable = true;

    # 自動配置 NixOS GPG Agent 與 pinentry-curses（解決 NixOS 終端解密卡住問題）
    enableGpgAgent = true;

    # 可選：註冊 Systemd 背景守護行程常駐運行 Web 管理儀表板
    web = {
      enable = true;          # 預設 false，設為 true 啟動背景服務
      port = 3000;            # 監聽通訊埠
      openFirewall = false;   # 是否自動放行 NixOS 防火牆
      user = "your_username"; # 執行服務的使用者
    };
  };
}
```

執行套用：
```bash
sudo nixos-rebuild switch
```

---

## 🏠 4. Home Manager 使用者級配置 (`home.nix`)

若您偏好透過 Home Manager 管理個人點文件與使用者軟體：

```nix
{ config, pkgs, inputs, ... }:

{
  imports = [
    inputs.cyber-note.homeManagerModules.default
  ];

  programs.cyber-note = {
    enable = true;
    # 自動建立 cn 快捷別名 (cn -> a)
    enableZshIntegration = true;
    enableBashIntegration = true;
    enableFishIntegration = true;
  };
}
```

---

## 🧩 5. 使用 Nixpkgs Overlay

若您希望在自己的 Flake 中直接使用 `pkgs.cyber-note` 或 `pkgs.a`：

```nix
{
  nixpkgs.overlays = [
    inputs.cyber-note.overlays.default
  ];
}
```
隨後即可在任何地方使用：
```nix
environment.systemPackages = [ pkgs.cyber-note ];
```

---

## 🎯 6. NixOS 100% 深度適配技術細節

| 挑戰項目 | 標準 Linux 做法 | Cyber-NOte NixOS 100% 適配方案 |
| :--- | :--- | :--- |
| **路徑依賴** | 假定 `/usr/bin/gpg` 存在 | `makeWrapper` 自動將 Nix store 中的 `gnupg`, `coreutils`, `which`, `xdg-utils`, `nodejs` 包裹至 PATH |
| **檔案不可變性** | 硬寫入 `/usr/bin/a` | 遵循 Nix Store 機制，產出乾淨的 `$out/bin/a` 與軟連結 `$out/bin/cyber-note` |
| **GPG 密碼輸入** | 依賴外部 GPG 守護程式 | 提供 `nixosModules` 自動調配 `programs.gnupg.agent` 與 `pinentry-curses` |
| **Rust 2024 Edition** | 依賴本機 rustc 版本 | 透過 `rust-overlay` 精準鎖定穩定版 Rust 1.85+ 工具鏈 |
| **Web 網頁後台** | 手動 npm run | 封裝成 `systemd.services.cyber-note-web`，支援隔離狀態目錄與權限加固 |
