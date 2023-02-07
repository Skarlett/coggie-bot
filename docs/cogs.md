# Cogs

cogs a way to extend the capabilities of coggiebot with any language. They are ran inside of a non-root user `nspawn` container. 

They contain the ability to fully utilize NixOS ecosystem.

Cogs are different than "Cogit". While "Cogit" is used to compile software written in any language, Cogs achieves the ability to compile software written in any language, and then ran as service. 


# Implementation
Cogs are simply NixOS configurations which are partially overloaded with default parameters, and then loaded into a NixOS container.

# Cog Security Levels
- **Trusted**
  Cogs which are labeled trusted are committed into the `$releaseBranch` branch by a maintainer of the project.
  
- **Staging**
  Cogs which are Staged will be restricted when using the discord API. They'll be locked into a private thread until merger into master, and only temporary.


# Using the discord API
coggiebot hosts an HTTP and WebSocket proxy on the network gateway. HTTP requests sent to it relay them to discord's api. coggiebot attempts to be API agnostic, if there's a confliction between the API layers, please open an issue.

coggiebot will replace the proxy requests, header's with its own Authorization Token. When submitting your API token, replace it with `COGGIEBOT_AUTH`

# Network
Coggiebot self-configures networks. **Trusted**, **Staging**, **Experimental**.

`mkCog` contains a parameter ``

- **Trusted** Cogs which are labeled as trusted can speak freely to other Cogs inside the local **Trusted network**. They're hosted under `${NetTrustAddr}/${NetTrustCidr}`


- **Staging** Cogs which are labeled as **Staging** maybe include a list of required Cogs to operate, but everything in the list must already included in Trusted

- **Experimental**
  Experimental cogs have access to trusted cogs with declarative dependency. They're  bandwidth 100mb.

Each cog's network lives in a Virtual Network which can speak with other cogs.  private network, where 

# Runtime Environment
Some custom environment variables are passed into the running processes context. The author doesn't need to use, or respect these rules.

- `ADMINS=1231312322:1342312321:adminId`
  if extends admin capabilities, this contains a list of administer discord IDs which can be used. The author of the Cog will be included.

- `COGPREFIX="~ @"`
  The `@` symbol signifies `@<botid>`, each prefix sequence is delimited via whitespace. All other characters considered literals. All prefixes are expected to be checked as the first sequence of characters in the message's body. 
  
- `BOTID=123123123`
  Cog's user ID

- `HTTP_PROXY="${NetworkGateway}"`
  Discord API proxy

- `USERNAME="coggerz"`
  Discord Bot name

# Intents, Commands & help menus
```nix
mkCog {
    name="cat-machine";

    commands = {
        cat = CogCommand {
            example = "cat";
            help = "fetches random image from cats api"
        };
    };
    
    extraHelp = ''
       reacting to any message with :bookmark: will send a copy to your direct message indox.
    '';
}

```
