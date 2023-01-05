#!/usr/bin/env sh
cat >> $out <<EOF
#!/bin/sh
$nix/bin/nix run --refresh github:skarlett/coggie-bot
EOF
chmod +x $out
