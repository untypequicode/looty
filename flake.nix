{
  description = "Looty — tasks and collaborative wiki (desktop client)";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAll = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAll (pkgs: rec {
        looty-desktop = pkgs.callPackage ./desktop/package.nix { };
        default = looty-desktop;
      });

      apps = forAll (pkgs: {
        default = { type = "app"; program = "${self.packages.${pkgs.system}.looty-desktop}/bin/looty"; };
      });

      devShells = forAll (pkgs: {
        # nix develop .#desktop
        desktop = pkgs.mkShell {
          packages = with pkgs; [ rustc cargo rustfmt clippy cargo-tauri pkg-config ];
          buildInputs = with pkgs; [ webkitgtk_4_1 libsoup_3 gtk3 glib-networking openssl librsvg ];
          shellHook = ''
            export GIO_MODULE_DIR=${pkgs.glib-networking}/lib/gio/modules
            export LD_LIBRARY_PATH=${pkgs.lib.makeLibraryPath [ pkgs.libayatana-appindicator ]}''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}
          '';
        };
      });
    };
}
