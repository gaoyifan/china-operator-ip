#!/usr/bin/env bash

rm ip-lists/*.txt
mv result/* ip-lists
cd ip-lists
tree -H . -P "*.txt|stat" -T "China Operator IP - prebuild results" > index.html
git config user.name "GitHub Actions"
git config user.email noreply@github.com
git add .
git commit -m "update $(date +%Y-%m-%d)"
git push -q
