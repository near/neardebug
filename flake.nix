{
    description = "Near Debugger Development environment";

    inputs = {
        nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
        flake-utils.url = "github:numtide/flake-utils";
    };

    outputs = { self, nixpkgs, flake-utils }: flake-utils.lib.eachDefaultSystem (system: rec {
        devShells.default = with nixpkgs.legacyPackages.${system}; mkShell {
            buildInputs = [
                miniserve
                wasm-pack
                wasm-bindgen-cli
                gcc
                typescript-language-server
                rustup
            ];
        };
    });
}
