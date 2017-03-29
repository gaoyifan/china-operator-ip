#!/usr/bin/env bash

source common.sh
set -e
[[ $SKIP_DATA_PREPARATION != true ]] && prepare_data
mkdir -p result
for file in operator/*.conf; do
	operator=${file%.*}
	operator=${operator##*/}
	log_info "generating IP list of $operator ..."
	get_asn $file | tee /dev/stderr | xargs bgptools | docker run -i --rm gaoyifan/cidrmerge > result/$operator.txt
	log_info "done"
done
