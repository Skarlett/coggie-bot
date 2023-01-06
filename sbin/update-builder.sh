#!/usr/bin/env sh
###################
# builder.sh

mkdir -p $out/bin
cat >> $out/bin/$name <<EOF
#!/usr/bin/env bash
###################
# lazy script

if [[ \$1 == "--debug" || \$1 == "-d" ]]; then
  echo "DEBUG ON"
  set -xe
fi

LOCKFILE=/tmp/coggiebot.update.lock
touch \$LOCKFILE
exec {FD}<>\$LOCKFILE

if ! flock -x -w 1 \$FD; then
  echo "Failed to obtain a lock"
  echo "Another instance of `basename \$0` is probably running."
  exit 1
else
  echo "Lock acquired"
fi

#
# Fetch latest commit origin/$branch
#
FETCH_DIR=\$(mktemp -d -t "coggie-bot.update.XXXXXXXX")
pushd \$FETCH_DIR
git init .
git remote add origin $origin_url
git fetch origin $branch
LHASH=\$(git show -s --pretty='format:%H' origin/$branch | sort -r | head -n 1)
popd
rm -rf \$FETCH_DIR
CHASH=\$(${coggiebot}/bin/coggiebot --built-from --token "")

#
# Dont replace canary (in source build)
#
if [[ \$CHASH == "canary" || \$LHASH == "canary" ]]; then
    echo "canary build -- nonapplicable"
    exit 0
fi

if [[ "\$CHASH" != "\$LHASH" ]]; then
  echo "start migrating"
  ${install_dir}/result/disable

  ${nix}/bin/nix build --refresh --out-link ${install_dir}/result github:skarlett/coggie-bot/$branch
  ${install_dir}/result/enable
  /bin/systemctl daemon-reload

  systemctl start ${coggiebotd}
  systemctl start ${coggiebotd-update-timer}
  echo "migrating finished"
fi

rm -f \$LOCKFILE
EOF
chmod +x $out/bin/$name
