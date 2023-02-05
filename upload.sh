#!/usr/bin/env bash

rm ip-lists/*.txt
mv result/* ip-lists
cd ip-lists
git config user.name Github Actions
git config user.email github-actions@github.com
git add .
git commit -m "update $(date +%Y-%m-%d)"
git push -q 

