#!/usr/bin/env bash

source common.sh
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
