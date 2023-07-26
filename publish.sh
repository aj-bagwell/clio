#!/bin/bash
set -euo pipefail

dir=$(dirname $0)

$dir/actions.sh

git push

cargo publish
v=$(sed -nr 's/^version = "([0-9.]+)"$/\1/p' Cargo.toml)
git tag -a "v$v" -m "Release version $v";
git push origin v$v
