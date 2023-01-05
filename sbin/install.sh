#!/usr/bin/env bash
###################
# bootstrap
######

INSTALL_USER="coggiebot"
INSTALL_DIR="/var/coggiebot"
NIXGRP="nixstore"
SERVICE="coggiebotd"
SERVICE_UPDATER="coggiebot-updaterd"
BRANCH="master"

if [[ ! -e "/nix/store" ]]; then
    echo "/nix/store not found";
    exit 1;
fi

if [ "$EUID" -ne 0 ]
  then echo "Please run as root"
  exit 1
fi

nix registry add coggiebot github:skarlett/coggiebot/$BRANCH

addgroup nixstore
chown :nixstore -R /nix/store

mkdir -p $INSTALL_DIR
chown -R $INSTALL_USER $INSTALL_DIR

groupadd -f $NIXGRP
adduser -m -G $NIXGRP $INSTALL_USER
usermod -aG $NIXGRP $USER

cat >> /etc/sudoers.d/coggiebot <<EOF
$INSTALL_USER ALL= NOPASSWD: /bin/systemctl restart $SERVICE.service
EOF



echo "todo: add .env variable to $INSTALL_DIR"
