#!/usr/bin/env bash

if [[ -f /etc/kern_loader.conf ]]; then
	alias grep='ggrep'
fi

get_asn(){
	CONF_FILE=$1
	unset PATTERN
	unset COUNTRY
	source $CONF_FILE
	grep -P "${COUNTRY}\$" asnames.txt | 
	grep -Pi "$PATTERN" | 
	awk '{print $1}' | 
	while read ASN; do 
		ASN=${ASN#AS}
		if grep '\.' <(echo $ASN) > /dev/null; then 
			ASN_HIGH=${ASN%.*}
			ASN_LOW=${ASN#*.}
			ASN=$((ASN_HIGH * 65536 + ASN_LOW))
		fi
		echo $ASN
	done
}

prepare_data(){
	wget http://bgp.potaroo.net/as1221/asnames.txt
	wget http://archive.routeviews.org/dnszones/rib.bz2
	docker run -it --rm -v `pwd`:/bgpdump -w /bgpdump gaoyifan/bgpdump bgpdump -m -O rib.txt rib.bz2
}
set -e
[[ $DEBUG != true ]] && prepare_data
mkdir -p result
find operator -type f -name '*.conf' |
while read file; do
	operator=${file%.*}
	operator=${operator##*/}
	get_asn $file | xargs bgptools | docker run -i --rm gaoyifan/cidrmerge > result/$operator.txt
done
