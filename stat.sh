#!/usr/bin/env bash

source common.sh
cd result
for file in *.txt; do 
	echo ${file%.*}
	if [[ $file == *6.txt ]]; then
		cat $file |
		awk -F\/ '{print $2}' |
		(
			sum=0
			while read n; do
				if [[ -n $n ]]; then
					((s=64-n))
					((sum+=1<<s))
				fi
			done
			echo $sum
			)
	else
		cat $file | 
		awk -F\/ '{print $2}' | 
		(
			sum=0
			while read n; do
				if [[ -n $n ]]; then
					((s=32-n))
					((sum+=1<<s))
				fi
			done
			echo $sum
		)
	fi
	echo
done | tee stat
