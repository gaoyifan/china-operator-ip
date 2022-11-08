#!/usr/bin/env bash

set -e

cidr-merger --version || \
    go install github.com/zhanhb/cidr-merger@v1.1.2

bgptools --version | grep -F $BGPTOOLS_VERSION || \
    cargo install --version $BGPTOOLS_VERSION bgptools

cidr-merger --version
bgptools --version
