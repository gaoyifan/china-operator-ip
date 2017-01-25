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
	>&2 echo "runing bgpdump ..."
	docker run -it --rm -v `pwd`:/bgpdump -w /bgpdump gaoyifan/bgpdump bgpdump -m -O rib.txt rib.bz2
	>&2 echo "done"
}
set -e
[[ $DEBUG != true ]] && prepare_data
mkdir -p result
find operator -type f -name '*.conf' |
while read file; do
	operator=${file%.*}
	operator=${operator##*/}
	>&2 echo "generating IP list of $operator ..."
	get_asn $file | xargs bgptools | docker run -i --rm gaoyifan/cidrmerge > result/$operator.txt
	>&2 echo "done"
done

# count the number of IP
cd result
ls *.txt | 
while read file; do 
	echo ${file%.*}
	cat $file | 
	awk -F\/ '{print $2}' | 
	(
		sum=0
		while read n; do
			((s=32-n))
			((sum+=1<<s))
		done
		echo $sum
	)
	echo
done > stat
