# coggie-bot
Hi! This is an open source discord bot written in rust. 
The goals of this project is to create a discord bot by our users. 

## Run
```sh
DISCORD_TOKEN=XXX nix run github:skarlett/coggie-bot
```

## Build

#### native
``` nix
nix build github:skarlett/coggie-bot
```

#### cross compilation
``` nix
# Show compilation options
nix flake show github:skarlett/coggie-bot

# cross compile
nix build github:skarlett/coggie-bot#packages.aarch64-linux
```

## Develpoment
``` nix
git clone https://github.com/skarlett/coggie-bot
cd coggie-bot
nix develop
```

#### updating dependencies
``` nix
cargo update
nix flake update
nix build
git commit -a -m "update dependencies"
git push origin your-update-branch
```

## Add to NixOS as flake
```nix
{
  description = "NixOS configuration";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-22.05";
    nixpkgs-unstable.url = "nixpkgs/nixos-unstable";
    coggiebot.url = "github:skarlett/coggie-bot";
  };

  outputs = { self, nixpkgs, coggiebot }:
    let
      system = "x86_64-linux";
    in {
      nixosConfigurations.hostname = nixpkgs.lib.nixosSystem {
        inherit system;
        modules = [
          # ...
          coggiebot.nixosModules.coggiebot
        ];
        
        services.coggiebot.enable = true;
        services.coggiebot.api-key = "XXXXXX";
      };
  };
}
```

