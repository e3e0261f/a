{
  description = "flake for github.com/e3e0261f/a (Rust project)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-23.11";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        rustPkgs = pkgs.rustPackages;
      in {
        packages.default = rustPkgs.buildRustPackage {
          pname = "a";
          version = "0.1.0";
          src = ./.;
          # 如需 release 构建： cargoBuildOptions = [ "--release" ];
        };

        devShells.default = pkgs.mkShell {
          buildInputs = [
            pkgs.rustc
            pkgs.cargo
            pkgs.rust-analyzer
          ];
          shellHook = ''
            echo "Dev shell for ${system} — cargo / rustc available"
          '';
        };

        defaultPackage = self.packages.${system}.default;
      });
}
