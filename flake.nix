{
  description = "Looty — tasks and collaborative wiki (desktop client)";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAll = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAll (pkgs: rec {
        looty-desktop = pkgs.callPackage ./desktop/package.nix { };
        default = looty-desktop;
      });

      apps = forAll (pkgs: {
        default = {
          type = "app";
          program = "${self.packages.${pkgs.system}.looty-desktop}/bin/looty";
        };
      });

      devShells = forAll (
        pkgs:
        let
          # SDK Android (licences acceptées explicitement pour ce shell uniquement).
          apkgs = import nixpkgs {
            system = pkgs.stdenv.hostPlatform.system;
            config = {
              allowUnfree = true;
              android_sdk.accept_license = true;
            };
          };
          sdk =
            (apkgs.androidenv.composeAndroidPackages {
              platformVersions = [
                "36"
                "37.0"
              ];
              buildToolsVersions = [ "37.0.0" ];
              includeEmulator = pkgs.stdenv.hostPlatform.isx86_64;
              includeSystemImages = pkgs.stdenv.hostPlatform.isx86_64;
              systemImageTypes = [ "default" ];
              abiVersions = [ "x86_64" ];
            }).androidsdk;
        in
        {
          # nix develop .#android
          android = pkgs.mkShell {
            packages = [
              sdk
              pkgs.jdk17
              pkgs.gradle_9
            ];
            ANDROID_HOME = "${sdk}/libexec/android-sdk";
            ANDROID_SDK_ROOT = "${sdk}/libexec/android-sdk";
            JAVA_HOME = pkgs.jdk17.home;
            # aapt2 téléchargé par Gradle ne fonctionne pas sous NixOS : celui du SDK.
            GRADLE_OPTS = "-Dorg.gradle.project.android.aapt2FromMavenOverride=${sdk}/libexec/android-sdk/build-tools/37.0.0/aapt2";
          };

          # nix develop .#desktop
          desktop = pkgs.mkShell {
            packages = with pkgs; [
              rustc
              cargo
              rustfmt
              clippy
              cargo-tauri
              pkg-config
            ];
            buildInputs = with pkgs; [
              webkitgtk_4_1
              libsoup_3
              gtk3
              glib-networking
              openssl
              librsvg
            ];
            shellHook = ''
              export GIO_MODULE_DIR=${pkgs.glib-networking}/lib/gio/modules
              export LD_LIBRARY_PATH=${
                pkgs.lib.makeLibraryPath [ pkgs.libayatana-appindicator ]
              }''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}
            '';
          };
        }
      );
    };
}
