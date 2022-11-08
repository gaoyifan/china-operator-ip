#!/bin/bash

[[ $(wc -l result/china.txt) < 3000 ]] && exit 1

[[ $(wc -l result/china6.txt) < 1000 ]] && exit 2

exit 0
