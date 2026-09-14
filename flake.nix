{
  description = "Cyber-NOte: 100% NixOS 深度適配之零成本抽象 GPG 密碼學審計與雲端同步筆記系統";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, ... }:
    let
      # 系統無關模組導出 (NixOS 與 Home Manager 模組)
      nixosModule = import ./nix/nixos-module.nix;
      homeManagerModule = import ./nix/home-manager-module.nix;
    in
    {
      # 🛡️ 1. 全域 NixOS 模組輸出 (直接在 configuration.nix 的 imports 引入)
      nixosModules = {
        default = nixosModule;
        cyber-note = nixosModule;
        a = nixosModule;
      };

      # 🏠 2. 全域 Home Manager 模組輸出 (可用於 home.nix)
      homeManagerModules = {
        default = homeManagerModule;
        cyber-note = homeManagerModule;
      };

      # 🧩 3. Nixpkgs Overlay 輸出 (允許無縫覆蓋至 pkgs.cyber-note 與 pkgs.a)
      overlays.default = final: prev: {
        cyber-note = final.callPackage ./nix/package.nix { };
        a = final.cyber-note;
      };
    }
    //
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) self.overlays.default ];
        };

        # 確保 Rust 工具鏈支援 Edition 2024 (Rust 1.85+)
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" "rustfmt" "clippy" ];
        };

        customRustPlatform = pkgs.makeRustPlatform {
          cargo = rustToolchain;
          rustc = rustToolchain;
        };

        cyberNotePkg = pkgs.callPackage ./nix/package.nix {
          rustPlatform = customRustPlatform;
        };
      in
      {
        # 📦 4. 軟體包輸出 (支援 nix build .#a 或 nix profile install)
        packages = rec {
          default = a;
          a = cyberNotePkg;
          cyber-note = a;
        };

        # 🚀 5. 即時執行 App (支援 nix run . 或 nix run .#web)
        apps = rec {
          default = a;
          a = flake-utils.lib.mkApp {
            drv = self.packages.${system}.a;
            name = "a";
          };
          cyber-note = a;
          web = flake-utils.lib.mkApp {
            drv = pkgs.writeShellScriptBin "cyber-note-web" ''
              exec ${self.packages.${system}.a}/bin/a --web "$@"
            '';
            name = "cyber-note-web";
          };
        };

        # 🛠️ 6. 純淨開發隔離環境 (支援 nix develop)
        devShells.default = pkgs.mkShell {
          name = "cyber-note-dev";

          nativeBuildInputs = [
            rustToolchain
            pkgs.pkg-config
          ];

          buildInputs = [
            pkgs.openssl
            pkgs.gnupg
            pkgs.pinentry-curses
            pkgs.git
            pkgs.nodejs_22
            pkgs.nodePackages.npm
            pkgs.xdg-utils
          ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin (
            if pkgs ? darwin && pkgs.darwin ? apple_sdk then [
              pkgs.darwin.apple_sdk.frameworks.Security
              pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
            ] else [ ]
          );

          shellHook = ''
            export RUST_SRC_PATH="${rustToolchain}/lib/rustlib/src/rust/library"
            export PKG_CONFIG_PATH="${pkgs.openssl.dev}/lib/pkgconfig:$PKG_CONFIG_PATH"
            echo ""
            echo "╔══════════════════════════════════════════════════════════════╗"
            echo "║    🛡️  Cyber-NOte · NixOS 100% 深度適配純淨開發環境          ║"
            echo "╚══════════════════════════════════════════════════════════════╝"
            echo "  🦀 Rust 工具鏈 : $(rustc --version)"
            echo "  🔐 GnuPG 版本  : $(gpg --version | head -n1)"
            echo "  🌐 Node.js     : $(node --version)"
            echo "  💡 快速建置    : cargo build --release"
            echo "  🚀 測試運行    : cargo run -- --help"
            echo "  🌐 啟動網頁後台: cargo run -- --web"
            echo ""
          '';
        };
      }
    );
}
