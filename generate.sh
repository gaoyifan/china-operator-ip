#!/usr/bin/env bash

source common.sh
set -e
[[ $SKIP_DATA_PREPARATION != true ]] && prepare_data
mkdir -p result
for file in operator/*.conf; do
	operator=${file%.*}
	operator=${operator##*/}
	log_info "generating IP list of $operator ..."
	get_asn $file
	get_asn $file | xargs bgptools -b rib.txt  | sort | uniq | docker run -i --rm yangzhaofengsteven/cidr-merge > result/${operator}.txt
	get_asn $file | xargs bgptools -b rib6.txt | sort | uniq | docker run -i --rm yangzhaofengsteven/cidr-merge > result/${operator}6.txt
done

