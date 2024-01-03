# Coggiebot configuration

coggiebot is configured via environment files.
- `DISCORD_TOKEN` is your discord token
- `RUST_LOG` Should be set to `'error,warn,info'`


### Example: On the go. (Best for short operations)
```sh
DISCORD_TOKEN='...' \
RUST_LOG='error,warn,info' \
nix run github:skarlett/coggie-bot
```

### Example: Install Linux. (Longer operations)
```sh
nix build github:skarlett/coggie-bot#deploy --out-link /opt/coggiebot
DISCORD_TOKEN='...' RUST_LOG='error,warn,info' /opt/coggiebot/bin/coggiebot
```

### Example: As Docker. 
```sh
nix build github:skarlett/coggie-bot#coggiebot-stable-docker
docker load < result
docker run containername:dev --env-file .coggiebot-env 
```


Each extension on coggiebot relies on its own set of environment variables.
