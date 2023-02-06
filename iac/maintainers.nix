{pkgs, lib}:
let
  mkMaintainer = {
    name
    , sshKeys ? []
    , github ? null
    , shell ? null
    , discordId ? null
  }:
  {
    inherit name github sshKeys discordId;
    #TODO: make this into a function
    profile =
      {
        inherit shell;
        packages=[ shell ];
        isNormalUser = true;
        description = "admin";
        extraGroups = [ "networkmanager" "wheel" ];
        openssh.authorizedKeys.keys = sshKeys;
        createHome = false;
      };
  };

  # adminKeys => ["key1" "key2"]
  adminKeys = admins: lib.foldl (x: y: x ++ y.sshKeys) [] admins;

  # admin => { <name> = { ... } }
  # evaluation ordered left to right,
  # FIXME: previous users may impersonate right foremost users.
  adminUsers = admins: lib.foldr (a: b: a.${b.name} // {${b.name} = b.profile;} ) {} admins;

  createAdmins = admins: lib.foldr (a: b:
    if (builtins.hasAttr [ b.name ] a)
    then a
    else (a.${b.name} // {${b.name} = b.profile;})) {};

in
rec {
  lunarix = mkMaintainer {
    name="lunarix";
    github="skarlett";
    sshKeys = [
      "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAILcon6Pn5nLNXEuLH22ooNR97ve290d2tMNjpM8cTm2r lunarix@masterbook"
    ];
    shell=pkgs.fish;
    discordId=191793436976873473;
  };

  admins = [
    lunarix
  ];

  adminUsers = createAdmins admins;
  adminKeys = adminKeys admins;
}
