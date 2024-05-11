{
  description = "basic rust development evnvironment";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    rust-overlay= {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-parts.url = "github:hercules-ci/flake-parts";
    systems.url = "github:nix-systems/default";
  };

  outputs = inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } {
      systems = import inputs.systems;
      # imports = [
      #   inputs.treefmt-nix.flakeModule
      # ];
      perSystem = { config, pkgs, lib, system, ... }:
      let 
        pkgs = import inputs.nixpkgs { inherit system; overlays = [ inputs.rust-overlay.overlays.default ]; };

        rustToolChain = (pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml);

        craneLib = (inputs.crane.mkLib pkgs).overrideToolchain rustToolChain;

        src = pkgs.lib.cleanSourceWith {
              src = ./.;
              filter = path: type:
                (pkgs.lib.hasSuffix "\.html" path) ||
                # Example of a folder for images, icons, etc
                (pkgs.lib.hasInfix "/public/" path) ||
                (pkgs.lib.hasInfix "/style/" path) ||
                # Default filter from crane (allow .rs files)
                (craneLib.filterCargoSources path type)
              ;
            };

        cargoToml = builtins.fromTOML (builtins.readFile (./Cargo.toml));

        args = {
              inherit src;
              pname = cargoToml.package.name;
              version = cargoToml.package.version;
              buildInputs = [
                pkgs.cargo-leptos
                pkgs.binaryen # Provides wasm-opt
                pkgs.openssl
                pkgs.pkg-config
              ];
            };

        cargoArtifacts = craneLib.buildDepsOnly args;

        buildArgs = args // {
            inherit cargoArtifacts;
            buildPhaseCargoCommand = "cargo leptos build --release -vvv";
            cargoTestCommand = "cargo leptos test --release -vvv";
            cargoExtraArgs = ""; # to remove the `--locked` default flag
            nativeBuildInputs = [
              pkgs.makeWrapper
            ];
            installPhaseCommand = ''
              mkdir -p $out/bin
              cp target/release/${cargoToml.package.name} $out/bin/
              cp -r target/site $out/bin/
              wrapProgram $out/bin/${cargoToml.package.name} \
                --set LEPTOS_SITE_ROOT $out/bin/site \
                --set LEPTOS_SITE_ADDR 127.0.0.1:3030 \
                --set BASE_URL http://127.0.0.1:3030 \
                --set BASE_WS_URL ws://127.0.0.1:3030
            '';
          };

          package = craneLib.buildPackage (buildArgs);
      in
    with pkgs; {
      devShells.default = mkShell {

          packages = [
            rustToolChain
            cargo-leptos
            sass
            leptosfmt
            binaryen # for wasm-opt
          ];
          
          nativeBuildInputs = [ ];
          
          buildInputs = with pkgs; [
            openssl
            pkg-config
          ];
        };

      packages.default = package;
    };
  };
}
