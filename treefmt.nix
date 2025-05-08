{ ... }:
{
  projectRootFile = "flake.nix";
  programs.nixfmt.enable = true;
  programs.rustfmt.enable = true;
  programs.toml-sort.enable = true;
}
