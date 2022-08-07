#!/usr/bin/env bash
# This script is meant to be run on Unix/Linux based systems
set -e

echo "*** Call RPC to get esg score"

while getopts c: flag
do
    case "${flag}" in
        c) company=${OPTARG};;
    esac
done

echo "Company: $company";
to_bytes() {
    local LC_CTYPE=C IFS=, n
    local -a cps
    # Iterate over each byte of the argument and append its numeric value to an array                                                                                                                                                            
    for (( n = 0; n < ${#1}; n++ )); do
        cps+=( $(printf "%d" "'${1:n:1}") )
    done
   res=$(printf "[%s]" "${cps[*]}")
   echo $res
}
to_bytes $company res 


# Define rpc call parameters
content="\"Content-Type: application/json;charset=utf-8"\"
json="\"jsonrpc"\"
version_json="\"2.0"\"
id="\"id"\"
method="\"method"\"
value="\"esg_querySustainable"\"
params="\"params"\"
# echo $res
entireUrl="curl http://localhost:9933 -H "$content" -d '{"$json":"$version_json", "$id":1, "$method":"$value","$params":[$res] }'"

echo $entireUrl
eval $entireUrl