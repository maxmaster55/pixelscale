{
  lib,
  rustPlatform,
  pkg-config,
  wrapGAppsHook4,
  gtk4,
  gtk4-layer-shell,
}:

rustPlatform.buildRustPackage {
  pname = "pixelscale";
  version = (lib.importTOML ../Cargo.toml).package.version;

  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      ../src
      ../data
      ../assets
    ];
  };

  cargoLock.lockFile = ../Cargo.lock;

  nativeBuildInputs = [ pkg-config wrapGAppsHook4 ];
  buildInputs = [ gtk4 gtk4-layer-shell ];

  postInstall = ''
    install -Dm644 data/io.github.maxmaster55.Pixelscale.desktop -t $out/share/applications
    install -Dm644 data/io.github.maxmaster55.Pixelscale.svg -t $out/share/icons/hicolor/scalable/apps
    install -Dm644 assets/fonts/*/*.ttf -t $out/share/fonts/truetype/pixelscale
  '';

  meta = {
    description = "A Tailscale panel in the Glowgrid pixel style";
    homepage = "https://github.com/maxmaster55/pixelscale";
    license = lib.licenses.gpl3Plus;
    mainProgram = "pixelscale";
    platforms = lib.platforms.linux;
  };
}
