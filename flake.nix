{
    description = "Near Debugger Development environment";

    inputs = {
        nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
        flake-utils.url = "github:numtide/flake-utils";
    };

    outputs = { self, nixpkgs, flake-utils }: flake-utils.lib.eachDefaultSystem (system:
    with nixpkgs.legacyPackages.${system};
    rec {
        # Run a local development webserver (`nix run`)
        apps.default = {
            type = "app";
            program = let script = writeScript "serve" ''
                #!${bash}/bin/bash
                ${miniserve}/bin/miniserve --spa --index index.html .
            ''; in "${script}";
        };
        # Open a development shell (`nix develop`)
        devShells.default = mkShell {
            buildInputs = [
                just
                wasm-pack
                wasm-bindgen-cli
                gcc
                typescript-language-server
                rustup
            ];
        };
    });
}
