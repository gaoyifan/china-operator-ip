#!/usr/bin/env bash

git remote set-url origin "https://${GITHUB_USERNAME}:${GITHUB_PASSWORD}@github.com/gaoyifan/china-operator-ip.git"
git add result
git commit -m "update `date +%Y-%m-%d`"
git push origin $(git rev-parse HEAD):${TRAVIS_BRANCH}
