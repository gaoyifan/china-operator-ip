#!/usr/bin/env bash

set -e

bgptools --version | grep -F $BGPTOOLS_VERSION || \
    cargo install --version $BGPTOOLS_VERSION bgptools

bgpkit-parser --version || \
    cargo binstall --secure --no-confirm bgpkit-parser@0.10.6

bgpkit-broker --version || \
    cargo binstall --secure --no-confirm bgpkit-broker@0.7.0

bgptools --version
bgpkit-parser --version
bgpkit-broker --version
