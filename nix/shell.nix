_: {
  perSystem =
    {
      pkgs,
      ...
    }:
    {
      devShells = {
        default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            rustc
            cargo
          ];
          RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
          LIBCLANG_PATH = with pkgs; "${llvmPackages.libclang.lib}/lib";

        };
      };
    };
}
