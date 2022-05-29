#!/usr/bin/env sh

parsed=$(free -h | awk 'NR == 2 {print $3; print $7} NR == 3 {print $3; print $4}')

mem_used=$(printf "${parsed}" | sed "1q;d")
mem_available=$(printf "${parsed}" | sed "2q;d")

swap_used=$(printf "${parsed}" | sed "3q;d")
swap_available=$(printf "${parsed}" | sed "4q;d")

if [ "${1}" = "mem" ]; then
    used="${mem_used}"
    available="${mem_available}"
elif [ "${1}" = "swap" ]; then
    used="${swap_used}"
    available="${swap_available}"
else
    echo "usage: ${0} mem|swap"
    exit 1
fi

# full_text
echo "${used}/${available}"
