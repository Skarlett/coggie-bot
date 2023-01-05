# coggie-bot
Hi! This is an open source discord bot written in rust.

## Features
| on-event | event-body | action taken                                |   |
|----------|------------|---------------------------------------------|---|
| reaction | 🔖         | dm message to reactor with copy of contents |   |
| message  | @version   | say package's version                       |   |
| message  | @rev       | say git hash built-from                     |   |
|          |            |                                             |   |

## Contributing
All contributions are welcome. When contributing, please pull request to a new branch, or use the `pull`. 
Add your name to the contributors.txt. Please describe the changes made, and add the features to the list above.

## Roadmap
- [X] Nix
- [ ] pre-commit hooks
- [ ] Automatic update delivery

## Run
```sh
DISCORD_TOKEN=XXX nix run github:skarlett/coggie-bot
```

## Build

#### native
```sh
nix build github:skarlett/coggie-bot
```

#### cross compilation
```sh
# Show compilation options
nix flake show github:skarlett/coggie-bot

# cross compile
nix build github:skarlett/coggie-bot#packages.aarch64-linux
```

## Develpoment
```sh
git clone https://github.com/skarlett/coggie-bot
cd coggie-bot
nix develop
```

#### updating dependencies
```sh
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
