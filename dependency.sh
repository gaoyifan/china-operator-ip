#!/usr/bin/env bash

set -e

cidr-merger --version || \
    go get github.com/zhanhb/cidr-merger@v1.1.2

bgptools --version | grep -F $BGPTOOLS_VERSION || \
    cargo install --vers $BGPTOOLS_VERSION bgptools

cidr-merger --version
bgptools --version
