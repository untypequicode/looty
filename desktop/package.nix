{ lib
, rustPlatform
, pkg-config
, wrapGAppsHook3
, webkitgtk_4_1
, libsoup_3
, gtk3
, glib-networking
, openssl
, librsvg
, libayatana-appindicator
, hunspellDicts
, runCommand
}:

let
  # Dictionnaires sous leur nom de langue : WebKitGTK demande « fr_FR » et « en_US », or le
  # dictionnaire français de nixpkgs s'appelle « fr-toutesvariantes » (introuvable sinon).
  dicts = runCommand "looty-hunspell-dicts" { } ''
    mkdir -p $out/share/hunspell
    for ext in aff dic; do
      ln -s ${hunspellDicts.fr-any}/share/hunspell/fr-toutesvariantes.$ext $out/share/hunspell/fr_FR.$ext
      ln -s ${hunspellDicts.en_US}/share/hunspell/en_US.$ext $out/share/hunspell/en_US.$ext
    done
  '';
in
rustPlatform.buildRustPackage {
  pname = "looty-desktop";
  version = (lib.importTOML ./src-tauri/Cargo.toml).package.version;

  src = lib.fileset.toSource {
    root = ./.;
    fileset = lib.fileset.unions [ ./src-tauri ./ui ./looty.desktop ];
  };
  cargoRoot = "src-tauri";
  buildAndTestSubdir = "src-tauri";
  cargoLock.lockFile = ./src-tauri/Cargo.lock;

  nativeBuildInputs = [ pkg-config wrapGAppsHook3 ];
  buildInputs = [ webkitgtk_4_1 libsoup_3 gtk3 glib-networking openssl librsvg ];

  # L'icône de la barre système charge libayatana-appindicator à l'exécution ;
  # le correcteur orthographique (enchant) trouve les dictionnaires par DICPATH (nuspell)
  # et XDG_DATA_DIRS/hunspell (hunspell).
  preFixup = ''
    gappsWrapperArgs+=(--prefix LD_LIBRARY_PATH : "${lib.makeLibraryPath [ libayatana-appindicator ]}")
    gappsWrapperArgs+=(--prefix DICPATH : "${dicts}/share/hunspell")
    gappsWrapperArgs+=(--prefix XDG_DATA_DIRS : "${dicts}/share")
  '';

  postInstall = ''
    install -Dm644 looty.desktop $out/share/applications/looty.desktop
    for s in 32x32 128x128; do
      install -Dm644 src-tauri/icons/$s.png $out/share/icons/hicolor/$s/apps/looty.png
    done
    install -Dm644 src-tauri/icons/128x128@2x.png $out/share/icons/hicolor/256x256/apps/looty.png
    install -Dm644 src-tauri/icons/icon.png $out/share/icons/hicolor/512x512/apps/looty.png
    install -Dm644 src-tauri/icons/icon.svg $out/share/icons/hicolor/scalable/apps/looty.svg
  '';

  meta = {
    description = "Looty desktop client: boards, tasks and collaborative wiki";
    mainProgram = "looty";
    platforms = lib.platforms.linux;
    license = lib.licenses.mit;
  };
}
