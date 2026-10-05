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
}:

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
  # le correcteur orthographique (enchant / hunspell) cherche ses dictionnaires dans DICPATH.
  preFixup = ''
    gappsWrapperArgs+=(--prefix LD_LIBRARY_PATH : "${lib.makeLibraryPath [ libayatana-appindicator ]}")
    gappsWrapperArgs+=(--prefix DICPATH : "${hunspellDicts.fr-any}/share/hunspell:${hunspellDicts.en_US}/share/hunspell")
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
