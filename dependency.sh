#!/usr/bin/env bash

set -ex

cidr-merger --version || {
    curl -sL -o ~/bin/gimme https://raw.githubusercontent.com/travis-ci/gimme/master/gimme
    chmod +x ~/bin/gimme
    eval "$(gimme stable)"
    go get github.com/zhanhb/cidr-merger
}
bgptools --version | grep -F $BGPTOOLS_VERSION || \
    cargo install --vers $BGPTOOLS_VERSION bgptools

cidr-merger --version
bgptools --version
