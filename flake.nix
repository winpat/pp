{
  description = "Command-line interface for the Parashift Platform API";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-23.11";
  };

  outputs = { self, nixpkgs }:
    let pkgs = nixpkgs.legacyPackages.x86_64-linux;
        deps = [
          pkgs.rustc
          pkgs.rust-analyzer
          pkgs.cargo
          pkgs.rustfmt
          pkgs.openssl
          pkgs.openssl.dev
          pkgs.pkg-config
          pkgs.clippy
        ];
    in {
      devShell.x86_64-linux = pkgs.mkShell {
        buildInputs = deps;
      };
      packages.x86_64-linux.pp = pkgs.rustPlatform.buildRustPackage rec {
        name = "pp";
        src = ./.;
        cargoLock = {
          lockFile = ./Cargo.lock;
        };
        buildInputs = deps;
        nativeBuildInputs = [ pkgs.pkgconfig ];
      };
      packages.x86_64-linux.default = self.packages.x86_64-linux.pp;

    };
}
