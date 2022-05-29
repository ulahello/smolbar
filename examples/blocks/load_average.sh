#!/usr/bin/env sh

load=$(cut -d ' ' -f1 /proc/loadavg)

# full_text
echo "$load"
