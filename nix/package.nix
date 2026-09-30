{
  lib,
  rustPlatform,
  pkg-config,
  dbus,
}:

let
  cargoToml = lib.importTOML ../Cargo.toml;
in
rustPlatform.buildRustPackage {
  pname = "nothingctl";
  inherit (cargoToml.package) version;

  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      ../src
      ../docs/protocol.md
    ];
  };

  cargoLock.lockFile = ../Cargo.lock;

  # bluer links libdbus.
  nativeBuildInputs = [ pkg-config ];
  buildInputs = [ dbus ];

  meta = {
    description = "Control Nothing and CMF audio devices over Bluetooth";
    homepage = "https://github.com/FormalSnake/nothingctl";
    license = lib.licenses.mit;
    mainProgram = "nothingctl";
    platforms = lib.platforms.linux;
  };
}
