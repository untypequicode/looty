{ lib, rustPlatform }:

rustPlatform.buildRustPackage {
  pname = "looty-bridge";
  version = (lib.importTOML ./Cargo.toml).package.version;

  src = lib.fileset.toSource {
    root = ./.;
    fileset = lib.fileset.unions [ ./Cargo.toml ./Cargo.lock ./src ./tests ];
  };
  cargoLock.lockFile = ./Cargo.lock;

  meta = {
    description = "Connects a PC to a Looty instance (Jupyter, LaTeX... through an outgoing tunnel)";
    homepage = "https://github.com/untypequicode/looty";
    license = lib.licenses.gpl3Plus;
    mainProgram = "looty-bridge";
  };
}
