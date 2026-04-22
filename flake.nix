{
  description = "A Nix flake for my reading notes static site generator";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    # This helper automatically handles x86_64-linux, aarch64-darwin, etc.
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
      in
      {
        devShells.default = pkgs.mkShell {
          # Tools available in the shell
          packages = [
            pkgs.cargo
            pkgs.rustc
            pkgs.rust-analyzer
            pkgs.git
            pkgs.just
          ];

          # Tells rust-analyzer where to find the standard library source
          RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";

          shellHook = ''
            echo "🦀 Rust development environment loaded for ${system}"
            echo "Standard library source: $RUST_SRC_PATH"
          '';
        };
      }
    );
}