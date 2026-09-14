{ config, lib, pkgs, ... }:

with lib;

let
  cfg = config.programs.cyber-note;
in {
  options.programs.cyber-note = {
    enable = mkEnableOption "Cyber-NOte (a) 密碼學安全筆記與雲端同步系統 (Home Manager 使用者級整合)";

    package = mkOption {
      type = types.package;
      default = pkgs.cyber-note or (pkgs.callPackage ./package.nix { });
      defaultText = literalExpression "pkgs.cyber-note";
      description = "安裝的 Cyber-NOte 軟體包。";
    };

    enableBashIntegration = mkOption {
      type = types.bool;
      default = true;
      description = "是否為 Bash 自動建立 cn (Cyber-NOte) 快捷別名。";
    };

    enableZshIntegration = mkOption {
      type = types.bool;
      default = true;
      description = "是否為 Zsh 自動建立 cn (Cyber-NOte) 快捷別名。";
    };

    enableFishIntegration = mkOption {
      type = types.bool;
      default = true;
      description = "是否為 Fish 自動建立 cn (Cyber-NOte) 快捷別名。";
    };
  };

  config = mkIf cfg.enable {
    # 注入使用者本機 PATH
    home.packages = [ cfg.package ];

    programs.bash.shellAliases = mkIf cfg.enableBashIntegration {
      cn = "a";
    };

    programs.zsh.shellAliases = mkIf cfg.enableZshIntegration {
      cn = "a";
    };

    programs.fish.shellAliases = mkIf cfg.enableFishIntegration {
      cn = "a";
    };
  };
}
