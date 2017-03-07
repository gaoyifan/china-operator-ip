#!/usr/bin/env bash

git config user.name $GIT_USER_NAME
git config user.email $GIT_USER_EMAIL

git clone -b ip-lists https://${GH_REF} ip-lists
mv result/* ip-lists
cd ip-lists
git add .
git commit -m "update $(date +%Y-%m-%d)"
git push -q "https://${GH_TOKEN}@${GH_REF}" ip-lists:ip-lists

