{
  description = "Pixelscale: a Tailscale panel in the Glowgrid pixel style";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAll = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAll (pkgs: rec {
        pixelscale = pkgs.callPackage ./nix/package.nix { };
        default = pixelscale;
      });

      # `nix develop`, then `cargo run`.
      devShells = forAll (pkgs: {
        default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [ cargo rustc clippy rustfmt pkg-config ];
          buildInputs = with pkgs; [ gtk4 gtk4-layer-shell ];
        };
      });
    };
}
