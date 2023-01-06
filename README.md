# coggie-bot
Hi! This is an open source discord bot written in rust.

## Controls
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
DISCORD_TOKEN=XXX nix run github:skarlett/coggie-bot#coggiebot
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


#### continuous integration on debian
the objective of using a custom package manager is to achieve the goal of self-updating.

``` sh
# jump to root
sudo su

# install nix-multiuser-mode
sh <(curl -L https://nixos.org/nix/install) --daemon

# activate PATH
# This is automatically appended into ~/.bashrc 
. ~/.nix-profile/etc/profile.d/nix.sh

echo "experimental-features = nix-command flakes" >> /etc/nix/nix.conf

adduser coggiebot

mkdir -p /var/coggiebot
chown coggiebot /var/coggiebot

###
# this pipeline does an inplace replacement  
echo "DefaultTimeoutStartSec=9999s" >> /etc/systemd/system.conf

su coggiebot



/result/activate
```


