#!/bin/bash
set -euo pipefail

dir=$(dirname $0)
actions="$dir/.github/workflows/rust.yml"
bold=$(tput bold)
normal=$(tput sgr0)

for step in $(yq '.jobs.build.steps[] | select(.run) | @json |  @base64' "$actions"); do
    name=$(echo $step | base64 -d | jq -r .name)
    run=$(echo $step | base64 -d | jq -r .run)
    echo "$bold==== $name ====$normal"
    $run
done

