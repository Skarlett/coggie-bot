{ config, lib, pkgs, ... }:
let
  mkDeveloper = {
    discord-id
    , github ? null
    , desc ? "NA"
    , languages ? []
  }:
  {
    inherit github discord-id desc languages;
  };
in
{
  maintainers = {
    lunarix = (mkDeveloper {
        github = "Skarlett";
        discord-id = 10293123910391039;
        desc = "leading shipwreck";
        languages = ["nix" "rust" "python"];
    });
  };
  repository = "github:Skarlett/coggie-bot";
  platforms = lib.platforms.linux;
  license = lib.licenses.bsd2;
}
