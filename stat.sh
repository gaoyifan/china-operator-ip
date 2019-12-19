#!/usr/bin/env bash

source common.sh
cd result
for file in *.txt; do
	echo ${file%.*}
	if [[ $file == *6.txt ]]; then
        #statistics IPv6 /48 blocks
        base=48
    else
        base=32
    fi
	cat $file |
	awk -F\/ '{print $2}' |
	(
		sum=0
		while read n; do
			if [[ -n $n ]] && [[ $n -le $base ]]; then
				((s=base-n))
				((sum+=1<<s))
			fi
		done
		echo $sum
	)
	echo
done | tee stat
