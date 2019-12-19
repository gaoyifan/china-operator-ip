#!/usr/bin/env bash

source common.sh
set -e
[[ $SKIP_DATA_PREPARATION != true ]] && prepare_data
mkdir -p result
for file in operator/*.conf; do
	operator=${file%.*}
	operator=${operator##*/}
	log_info "generating IP list of $operator ..."
	get_asn $file | tee /dev/stderr | xargs bgptools -b rib.txt | sort | uniq | docker run -i --rm yangzhaofengsteven/cidr-merge > result/$operator.txt
	log_info "done"
done

for file in operator6/*.conf; do
	operator=${file%.*}
	operator=${operator##*/}
	log_info "generating IPv6 list of $operator ..."
	get_asn $file | tee /dev/stderr | xargs bgptools -b rib6.txt | sort | uniq | docker run -i --rm yangzhaofengsteven/cidr-merge > result/${operator}6.txt
	log_info "done"
done
