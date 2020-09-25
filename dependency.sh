#!/usr/bin/env bash

set -e

cidr-merger --version || {
    curl -sL -o ~/bin/gimme https://raw.githubusercontent.com/travis-ci/gimme/master/gimme
    chmod +x ~/bin/gimme
    eval "$(gimme stable)"
    go get github.com/zhanhb/cidr-merger
}
asroute 0 < /dev/null || {
    git clone https://github.com/yangzhaofeng/aspathanalysis.git
    cd aspathanalysis
    make && make install
}

cidr-merger --version
asroute 0 < /dev/null
