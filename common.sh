#!/usr/bin/env bash

[[ $DEBUG == true ]] && set -x

log_info(){
	>&2 echo "INFO>" $@
}

prepare_data(){
	curl -sSLo latest-bview.gz https://data.ris.ripe.net/rrc00/latest-bview.gz
	log_info "runing bgpdump ..."
	bgpdump -m -O ${BVIEW_PATH}latest-bview.txt latest-bview.gz
	grep -v '::/' < ${BVIEW_PATH}latest-bview.txt > rib.txt & # prepare_data_v4
	grep '::/' < ${BVIEW_PATH}latest-bview.txt > rib6.txt & # prepare_data_v6
	wait_exit
}

wait_exit(){
	local oldstate=$(set +o)
	set +e
	local s=0
	while [[ $s -ne 127 ]]; do
		[[ $s -ne 0 ]] && exit $s
		wait -n
		s=$?
	done
	eval "$oldstate"
	return 0
}
