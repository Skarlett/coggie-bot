#!/usr/bin/env bash
###################
# lazy cronjob script

set -e
if [[ $1 -eq "--debug" || $1 -eq "-d" ]]; then
  echo "DEBUG ON"
  set -x
fi

#
# Fetch latest commit origin/master
#
FETCH_DIR=$(mktemp -d -T "coggie-bot.update")
pushd $FETCH_DIR
git init .
git remote add origin https://github.com/Skarlett/coggie-bot.git
git fetch origin master

LHASH=$(git show -s --pretty='format:%H' origin/master | sort -r | head -n 1)
popd

rm -rf $FETCH_DIR

CHASH=$(coggiebot --built-from)

#
# Dont replace canary (in source build)
#
if [[ $CHASE -eq "canary" || $LHASH -eq "canary" ]]; then
    echo "canary build -- nonapplicable"
    exit 0
fi

if [[ $CHASE -ne $LHASH ]]; then
  killall coggiebot-agent.sh
  rm -f /tmp/coggiebot.lock
fi

#
# run only once
#
(flock -xn 9 -w 0 || exit 1
  nix run --refresh github:skarlett/coggie-bot
) 9>/tmp/coggiebot.lock
