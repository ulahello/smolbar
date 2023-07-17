{ lib, rustPlatform, scdoc, gnumake }:
let manifest = (lib.importTOML ./Cargo.toml).package;
in
rustPlatform.buildRustPackage rec {
  pname = manifest.name;
  version = manifest.version;
  meta = with lib; {
    description = manifest.description;
    license = manifest.license;
    homepage = manifest.homepage;
    maintainers = manifest.authors;
    platforms = platforms.unix;
    badPlatforms = platforms.windows;
  };

  cargoLock.lockFile = ./Cargo.lock;
  src = lib.cleanSource ./.;

  nativeBuildInputs = [ gnumake scdoc ];

  postInstall = ''
    cd docs && PREFIX="$out" make install
  '';
}
