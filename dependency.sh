#!/usr/bin/env bash

set -e

cidr-merger --version || {
    go install github.com/zhanhb/cidr-merger@v1.1.3
}
asroute 0 < /dev/null || {
    git clone https://github.com/yangzhaofeng/aspathanalysis.git
    cd aspathanalysis
    make && sudo make install
}

cidr-merger --version
asroute 0 < /dev/null
