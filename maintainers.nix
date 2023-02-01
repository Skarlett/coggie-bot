{pkgs, lib}:
let
  mkMaintainer = {
    lib
    , name
    , sshKeys ? []
    , github ? null
    , shell ? null
  }:
  {
    inherit name github sshKeys;
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
  # previous users may impersonate right foremost users.
  adminUsers = admins: lib.foldr (a: b: a.${b.name} // {${b.name} = b.profile;} ) {} admins;
in
rec {
  lunarix = mkMaintainer {
    name="lunarix";
    github="skarlett";
    sshKeys = [
      "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAILcon6Pn5nLNXEuLH22ooNR97ve290d2tMNjpM8cTm2r lunarix@masterbook"
    ];
    shell=pkgs.fish;
  };

  admins = [
    lunarix
  ];

  adminUsers = adminUsers admins;
  adminKeys = adminKeys admins;
}
