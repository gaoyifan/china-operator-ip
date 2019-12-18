#!/usr/bin/env bash

[[ $DEBUG == true ]] && set -x

log_info(){
    >&2 echo "INFO>" $@
}

get_asn(){
	local CONF_FILE=$1
	unset PATTERN
	unset COUNTRY
	unset EXCLUDE
	source $CONF_FILE
	EXCLUDE=${EXCLUDE:-"^$"}
	grep -P "${COUNTRY}\$" asnames.txt |
	grep -Pi "$PATTERN" |
	grep -vPi "$EXCLUDE" |
	awk '{gsub(/AS/, ""); print $1 }'
}

prepare_data(){
	curl -sSL https://bgp.potaroo.net/cidr/autnums.html | awk '-F[<>]' '{print $3,$5}' | grep '^AS' > asnames.txt
	curl -sSLo rib.bz2 http://archive.routeviews.org/dnszones/rib.bz2
	log_info "runing bgpdump ..."
	docker run -it --rm -v `pwd`:/bgpdump -w /bgpdump gaoyifan/bgpdump bgpdump -m -O rib.txt rib.bz2
	log_info "done"
}
