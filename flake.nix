  devShell = pkgs.mkShell {
    packages = [
      pkgs.rust-bin.nightly."2023-04-30".default
      pkgs.rust-analyzer
      pkgs.samply
      nix-kani.packages.${system}.kani
    ];

    RUST_BACKTRACE = 1;
  };
