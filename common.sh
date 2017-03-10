#!/usr/bin/env bash

[[ $DEBUG == true ]] && set -x

log_info(){
    >&2 echo "INFO>" $@
}

get_asn(){
	local CONF_FILE=$1
	unset PATTERN
	unset COUNTRY
	source $CONF_FILE
	grep -P "${COUNTRY}\$" asnames.txt | 
	grep -Pi "$PATTERN" | 
	awk '{gsub(/AS/, ""); print $1 }'
}

prepare_data(){
	wget http://bgp.potaroo.net/as1221/asnames.txt -O asnames.txt
	wget http://archive.routeviews.org/dnszones/rib.bz2 -O rib.bz2
	log_info "runing bgpdump ..."
	docker run -it --rm -v `pwd`:/bgpdump -w /bgpdump gaoyifan/bgpdump bgpdump -m -O rib.txt rib.bz2
	log_info "done"
}
