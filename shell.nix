{ pkgs ? import <nixpkgs> { } }:

pkgs.mkShell {
  buildInputs = [
    pkgs.rustc
    pkgs.cargo
    pkgs.openssl
    pkgs.openssl.dev
    pkgs.pkgconfig
    pkgs.rustfmt
  ];
}
