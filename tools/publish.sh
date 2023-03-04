#!/bin/bash

set -exu

VERSION=$(grep "^version" ./rethabi/Cargo.toml | sed -e 's/.*"\(.*\)"/\1/')
ORDER=(rethabi derive contract cli)

echo "Publishing $VERSION"
cargo clean

for crate in ${ORDER[@]}; do
	echo "Publishing $crate@$VERSION"
	sleep 5
	cd $crate
	cargo publish $@
	cd -
done

echo "Tagging version $VERSION"
git tag -a v$VERSION -m "Version $VERSION"
git push --tags
