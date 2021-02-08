#!/usr/bin/env bash

git clone -b ip-lists https://${GH_REF} ip-lists
rm ip-lists/*.txt
mv result/* ip-lists
cd ip-lists
tree -H . -P "*.txt|stat" -T "China Operator IP - prebuild results" > index.html
git config user.name $GIT_USER_NAME
git config user.email $GIT_USER_EMAIL
git add .
git commit -m "update $(date +%Y-%m-%d)"
git push -q "https://${GH_TOKEN}@${GH_REF}" ip-lists:ip-lists

