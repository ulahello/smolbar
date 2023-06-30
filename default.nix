{ pkgs, lib, rustPlatform }:
let manifest = (lib.importTOML ./Cargo.toml).package;
in
rustPlatform.buildRustPackage rec {
  pname = manifest.name;
  version = manifest.version;
  cargoLock.lockFile = ./Cargo.lock;
  src = lib.cleanSource ./.;
  meta = with lib; {
    description = manifest.description;
    license = manifest.license;
    homepage = manifest.homepage;
    maintainers = manifest.authors;
    platforms = platforms.unix;
    badPlatforms = platforms.windows;
  };

  buildPhase = ''
cd doc
# TODO: fails unless documentation is already built (permission denied: scdoc < smolbar.1.scd > smolbar.1)
make clean all
  '';
  installPhase = ''
cd "$src/doc" && PREFIX="$out" make install
mkdir -p "$out/bin"
cp "$src/target/release/smolbar" "$out/bin"
  '';
  nativeBuildInputs = with pkgs.buildPackages; [ cargo gnumake scdoc ];
}
