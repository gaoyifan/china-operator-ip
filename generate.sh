#!/usr/bin/env bash

set -e
source common.sh
[[ $SKIP_DATA_PREPARATION != true ]] && prepare_data
mkdir -p result
for file in operator/*.conf; do
	operator=${file%.*}
	operator=${operator##*/}
	log_info "generating IP list of $operator ..."
	source $file
	if [ -z "AS6" ]; then
		AS6=$AS
	fi
	asroute $AS $EXCLUDE < rib.txt | cidr-merger -s > result/${operator}.txt &
	asroute $AS6 $EXCLUDE < rib6.txt | cidr-merger -s > result/${operator}6.txt &
	unset AS AS6 EXCLUDE
done
wait_exit

{
	cd result/
	ls ../operator/ | sed 's/\.conf/.txt/g' | xargs cat | cidr-merger -s > china.txt
	ls ../operator/ | sed 's/\.conf/6.txt/g' | xargs cat | cidr-merger -s > china6.txt
}
