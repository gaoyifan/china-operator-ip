#!/usr/bin/env bash

set -e
source common.sh
[[ $SKIP_DATA_PREPARATION != true ]] && prepare_data
mkdir -p result
for file in operator/*.conf; do
	operator=${file%.*}
	operator=${operator##*/}
	log_info "generating IP list of $operator ..."
	get_asn $file
	get_asn $file | xargs bgptools --mrt-file rib.gz   | grep -Fv : | cat > result/${operator}.txt  &
	get_asn $file | xargs bgptools --mrt-file rib6.bz2 | grep -v '^::/0$' | grep -F  : | cat > result/${operator}6.txt &
done

wait_exit
