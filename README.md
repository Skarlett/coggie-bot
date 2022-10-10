# coggie-bot
Hi! This is an open source discord bot written in rust.

## Limitation
*raspberry pi 3B+* 
| resource | max             |   |
|----------|-----------------|---|
| RAM:     | **512MB**       |   |
| CPU:     | 1.4GHz 64-bit   |   |
| CPU Arch | aarch64         |   |
| Net:     | 100mbps/100mbps |   |
| Storage: | 32GB            |   |
|          |                 |   |

## Features
| on-event | event-body | action taken          |   |
|----------|------------|-----------------------|---|
| reaction | 🔖         | dm message to reactor |   |
| message  | @version   | say package's version |   |
|          |            |                       |   |

## Contributing
All contributions are welcomed. When contributing, please pull request to a new branch, or use the `pull`. 
Add your name to the contributors.txt. Please describe the changes made, and add the features to the list above.

This project uses the `nix` package manager to provide a uniformed development enviroment and deployment. `nix` can be installed using the following

### Install Nix on Linux/MacOS
``` sh
curl -L https://nixos.org/nix/install | sh
```
[See repository](https://github.com/NixOS/nix)

#### Enable experimental-features
After you install nix, make sure to enable flakes, and future commands.
```
# ~/.config/nix/nix.conf
experimental-features = nix-command flakes
```

## Roadmap
- [X] Nix
- [ ] pre-commit hooks
- [ ] Workflow CI/CD
- [ ] cross compilation on CI/CD
- [ ] Automatic update delivery

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
