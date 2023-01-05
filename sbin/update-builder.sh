#!/usr/bin/env sh
###################
# builder.sh

bin=$coggiebot/bin/coggiebot

cat >> $out <<EOF
#!/usr/bin/env bash
###################
# lazy script

set -e
if [[ \$1 -eq "--debug" || \$1 -eq "-d" ]]; then
  echo "DEBUG ON"
  set -x
fi

#
# Fetch latest commit origin/master
#
FETCH_DIR=\$(mktemp -d -t "coggie-bot.update.XXXXXXXX")
pushd \$FETCH_DIR
git init .
git remote add origin https://github.com/Skarlett/coggie-bot.git
git fetch origin master
LHASH=\$(git show -s --pretty='format:%H' origin/master | sort -r | head -n 1)
popd
rm -rf \$FETCH_DIR

CHASH=\$( $bin --built-from )

#
# Dont replace canary (in source build)
#
if [[ \$CHASE -eq "canary" || \$LHASH -eq "canary" ]]; then
    echo "canary build -- nonapplicable"
    exit 0
fi

if [[ \$CHASE -ne \$LHASH ]]; then
  systemctl restart coggiebotd
fi
EOF

chmod +x $out
