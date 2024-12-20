{
  description = "Nix flake for the cwe_checker with patched Ghidra as a dependency.";

  inputs = {
    # Depend on NixOS-unstable for the latest Rust version.
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      pkgs = nixpkgs.legacyPackages."x86_64-linux";
      # Building Ghidra.
      ghidra-cwe-checker-plugin = pkgs.ghidra.buildGhidraScripts {
        pname = "cwe_checker";
        name = "cwe_checker";
        src = ./ghidra_plugin;
      };
      cwe-ghidra = pkgs.ghidra.withExtensions (p: with p; [ ghidra-cwe-checker-plugin ]);
      # Path to Java Ghidra plugin.
      cwe-checker-ghidra-plugins = pkgs.runCommand
        "cwe-checker-ghidra-plugins" { src = ./src/ghidra/p_code_extractor; }
        ''
        mkdir -p $out/p_code_extractor
        cp -rf $src/* $out/p_code_extractor
        '';
      # Build Ghidra package with analyzeHeadless in support/ instead of bin/.
      # This is where the cwe_checker expects it to be.
      cwe-ghidra-path-fix = pkgs.stdenv.mkDerivation {
        name = "analyzeHeadless";
        pname = "analyzeHeadless";
        buildInputs = [ cwe-ghidra ];
        src = cwe-ghidra;
        buildPhase = ''
        mkdir -p $out
        cp -rf ${cwe-ghidra} $out
        # cwe checker expects
        mkdir -p $out/support
        cp ${cwe-ghidra}/bin/ghidra-analyzeHeadless $out/support/analyzeHeadless
        '';
      };
      # Building cwe_checker.
      cwe-checker-bins = pkgs.rustPlatform.buildRustPackage {
        pname = "cwe_checker";
        name = "cwe_checker";
        src = ./.;
        cargoLock = {
          lockFile = ./Cargo.lock;
        };
      };
      # Build ghidra.json
      cwe-ghidra-json = pkgs.writeTextFile {
        name = "GhidraConfigFile";
        text = builtins.toJSON { ghidra_path = ''${cwe-ghidra-path-fix}''; };
      };
      # Creates config dir for cwe_checker.
      cwe-checker-configs = pkgs.runCommand "cwe-checker-configs" { src = ./src; }
      ''
      mkdir -p $out
      cp $src/config.json $out
      cp $src/lkm_config.json $out
      ln -s ${cwe-ghidra-json} $out/ghidra.json
      '';
      # Target bin for 'nix run'.
      cwe-checker = pkgs.writeScriptBin "cwe-checker" ''
      #!/bin/sh
      CWE_CHECKER_CONFIGS_PATH=${cwe-checker-configs} \
      CWE_CHECKER_GHIDRA_PLUGINS_PATH=${cwe-checker-ghidra-plugins} \
      ${cwe-checker-bins}/bin/cwe_checker $@;
      '';
    in
    {
      devShell.x86_64-linux = pkgs.mkShell {
        buildInputs = with pkgs; [
          rustc
          cargo
          cwe-ghidra-path-fix
        ];
        shellHook = ''
        export CWE_CHECKER_CONFIGS_PATH=${cwe-checker-configs} \
        export CWE_CHECKER_GHIDRA_PLUGINS_PATH=${cwe-checker-ghidra-plugins} \
        '';
      };
      packages.x86_64-linux.default = cwe-checker;
    };
}

