#!/bin/sh
# Runs on the server, from the deploy job, after it loads the image $1: points the app at it,
# starts whatever changed, and keeps the three newest app images. A rollback is
# `IMAGE=kokonoe:<sha> docker compose up -d` in /opt/kokonoe.
set -eu
cd "$(dirname "$0")"
sed -i '/^IMAGE=/d' .env
echo "IMAGE=$1" >> .env
docker compose up --detach --remove-orphans --wait
docker image ls kokonoe --format '{{.Tag}}' | tail -n +4 | xargs -r -I{} docker image rm kokonoe:{}
