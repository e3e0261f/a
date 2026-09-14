{ config, lib, pkgs, ... }:

with lib;

let
  cfg = config.services.cyber-note;
in {
  options.services.cyber-note = {
    enable = mkEnableOption "Cyber-NOte (a) 密碼學安全筆記與雲端同步系統";

    package = mkOption {
      type = types.package;
      default = pkgs.cyber-note or (pkgs.callPackage ./package.nix { });
      defaultText = literalExpression "pkgs.cyber-note";
      description = "使用的 Cyber-NOte 軟體包。";
    };

    enableGpgAgent = mkOption {
      type = types.bool;
      default = true;
      description = ''
        自動在 NixOS 啟用並正確調配 GnuPG Agent 與 pinentry-curses 互動式終端口令輸入。
        解決 NixOS 預設環境下 GPG 缺少 agent 導致加密/解密卡死的問題。
      '';
    };

    web = {
      enable = mkOption {
        type = types.bool;
        default = false;
        description = "是否將 Cyber-NOte Web 網頁端管理後台註冊為 Systemd 常駐守護行程服務。";
      };

      port = mkOption {
        type = types.port;
        default = 3000;
        description = "Cyber-NOte Web 儀表板監聽的 HTTP 埠號。";
      };

      openFirewall = mkOption {
        type = types.bool;
        default = false;
        description = "是否自動在 NixOS 防火牆 (networking.firewall) 放行 Web 管理埠。";
      };

      user = mkOption {
        type = types.str;
        default = "root";
        description = "執行 Web 服務的使用者帳號。建議設定為平常登入的非 root 帳戶。";
      };

      dataDir = mkOption {
        type = types.str;
        default = "/var/lib/cyber-note";
        description = "Cyber-NOte Web 服務的筆記工作目錄 (含 Gist 同步本機快取與金鑰資料)。";
      };
    };
  };

  config = mkIf cfg.enable {
    # 1. 注入全域系統指令：系統所有使用者皆可直接在終端執行 a 與 cyber-note
    environment.systemPackages = [ cfg.package ];

    # 2. 自動配置 NixOS GPG 代理 (落實 100% 隨開即用)
    programs.gnupg.agent = mkIf cfg.enableGpgAgent {
      enable = true;
      pinentryPackage = mkDefault pkgs.pinentry-curses;
      # Cyber-NOte 堅持密碼學分離，預設不劫持 SSH agent
      enableSSHSupport = mkDefault false;
    };

    # 3. 防火牆放行設定
    networking.firewall.allowedTCPPorts = mkIf (cfg.web.enable && cfg.web.openFirewall) [
      cfg.web.port
    ];

    # 4. Systemd 守護服務 (當 cfg.web.enable = true 時)
    systemd.services.cyber-note-web = mkIf cfg.web.enable {
      description = "Cyber-NOte Web Management Engine Daemon";
      after = [ "network.target" ];
      wantedBy = [ "multi-user.target" ];

      environment = {
        PORT = toString cfg.web.port;
        NODE_ENV = "production";
        HOME = cfg.web.dataDir;
      };

      serviceConfig = {
        Type = "simple";
        User = cfg.web.user;
        WorkingDirectory = cfg.web.dataDir;
        ExecStart = "${cfg.package}/bin/a --web --port ${toString cfg.web.port}";
        Restart = "on-failure";
        RestartSec = "5s";
        StateDirectory = "cyber-note";
        ProtectSystem = "strict";
        ProtectHome = "read-only";
        ReadWritePaths = [ cfg.web.dataDir ];
        PrivateTmp = true;
      };
    };
  };
}
