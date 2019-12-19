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

prepare_data_v4(){
	curl -sSLo rib.bz2 http://archive.routeviews.org/dnszones/rib.bz2
	log_info "runing bgpdump v4 ..."
	docker run -it --rm -v `pwd`:/bgpdump -w /bgpdump gaoyifan/bgpdump bgpdump -m -O rib.txt rib.bz2
	log_info "finish bgpdump v4"
}
prepare_data_v6(){
	IP6UPSTREAM="http://archive.routeviews.org/route-views6/bgpdata"
	MONTH6=$(lftp -e 'cls -1;exit' $IP6UPSTREAM  2>/dev/null | sort | tail -n 1)
	LATEST6=$(lftp -e 'cls -1;exit' $IP6UPSTREAM/$MONTH6/RIBS/  2>/dev/null | sort | tail -n 1)
	curl -sSLo rib6.bz2 "$IP6UPSTREAM/$MONTH6/RIBS/$LATEST6"
	log_info "runing bgpdump v6 ..."
	docker run -it --rm -v `pwd`:/bgpdump -w /bgpdump gaoyifan/bgpdump bgpdump -m -O rib6.txt rib6.bz2
	log_info "finish bgpdump v6"
}
prepare_data(){
    curl -sSL https://bgp.potaroo.net/cidr/autnums.html | awk '-F[<>]' '{print $3,$5}' | grep '^AS' > asnames.txt &
    prepare_data_v4 &
    prepare_data_v6 &
    wait
}
