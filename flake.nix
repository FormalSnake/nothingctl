{
  description = "nothingctl: control Nothing and CMF audio devices from the command line";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" "x86_64-darwin" ];
      linuxSystems = [ "x86_64-linux" "aarch64-linux" ];
      forAll = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = nixpkgs.lib.genAttrs linuxSystems (system: rec {
        nothingctl = nixpkgs.legacyPackages.${system}.callPackage ./nix/package.nix { };
        default = nothingctl;
      });

      overlays.default = final: _: { nothingctl = final.callPackage ./nix/package.nix { }; };

      devShells = forAll (pkgs: {
        default = pkgs.mkShell {
          packages = [ pkgs.cargo pkgs.rustc pkgs.clippy pkgs.rustfmt ]
            ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [ pkgs.pkg-config pkgs.dbus ];
        };
      });
    };
}
