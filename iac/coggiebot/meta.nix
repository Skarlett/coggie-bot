{ config, lib, pkgs, ... }:
let
  mkDeveloper = {
    discordid
    , github ? null
    , languages ? []
  }:
  {
    inherit github discordid languages;
  };
in
{
  maintainers = {
    lunarix = (mkDeveloper {
        github = "Skarlett";
        discordid = 10293123910391039;
        languages = ["nix" "rust" "python"];
    });
  };
  repository = "github:Skarlett/coggie-bot";
  platforms = lib.platforms.linux;
  license = lib.licenses.bsd2;
}
