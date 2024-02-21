{
  description = "fix Rust devShell";

  inputs = {
    nixpkgs.url      = "github:NixOS/nixpkgs/23.11";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-parts.url  = "github:hercules-ci/flake-parts";
  };

  outputs = { nixpkgs, rust-overlay, flake-parts, ... }:
    flake-parts.perSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
      in
      with pkgs;
      {
        devShells.default = mkShell {
          buildInputs = [
            openssl
            pkg-config
            rust-bin.beta.latest.default
          ];
        };
      }
    );
}
