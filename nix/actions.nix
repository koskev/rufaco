{ inputs, ... }:
{
  imports = [ inputs.actions-nix.flakeModules.default ];
  flake.actions-nix = {
    pre-commit.enable = true;
    defaultValues = {
      jobs = {
        runs-on = "ubuntu-latest";
      };
    };
    workflows = {
      #".github/workflows/linting.yaml" = inputs.nix-actions.lib.mkClippy { };
      ".github/workflows/build.yaml" = inputs.nix-actions.lib.mkBuild {
        extraBuildSteps = inputs.nix-actions.lib.mkCachixSteps { };
      };
    };
  };
}
