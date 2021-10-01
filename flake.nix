{
  description = "A basic flake with a shell";
  #inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  #inputs.nix.inputs.nixpkgs.follows = "nixpkgs";
  inputs.flake-utils.url = "github:numtide/flake-utils";
  inputs.rust-overlay.url = "github:oxalica/rust-overlay";

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system: let
      overlays = [ (import rust-overlay) ];
      pkgs = import nixpkgs {
        inherit system overlays;
        config.allowUnfree = true;
      };
      deps = with pkgs; [
          rust-bin.nightly.latest.default
          pkgconfig udev alsaLib lutris clang llvmPackages.lld
		      libGL gcc libudev
          x11 xorg.libXcursor xorg.libXrandr xorg.libXi
          xorg.libxcb python3 # required for xcb rust dep
          vulkan-tools vulkan-headers vulkan-loader vulkan-validation-layers
      ];
    in {
      devShell = pkgs.mkShell {
        nativeBuildInputs = [ pkgs.bashInteractive ];
        buildInputs = deps;

        LD_LIBRARY_PATH = with pkgs; lib.makeLibraryPath deps;
      };
    });
}
