#!/usr/bin/env sh

package_executable() {
    local temp_dir=$(mktemp -d)
    local name="$PROJECT_NAME-$TRAVIS_TAG-$TARGET"

    # Setting up the directory structure for packaging the program. 
    local staging="$temp_dir/$name"
    mkdir -p "$staging"
    mkdir -p "$staging/docs"
    cp {README.adoc,LICENSE} "$staging"
    cp {docs/manual.adoc,CHANGELOG.adoc,texture-notes.1} "$staging/docs"
    cp "target/$TARGET/release/texture-notes" "$staging"

    # This directory is where the binaries will be stored.
    local out_dir="$(pwd)/deployment"
    mkdir -p "$out_dir"

    # Creating the archive from the staging area.
    if [ "$TRAVIS_OS_NAME" = "windows" ]; then 
        local out_file="$name.7z"
        7z a -t7z "$out_dir/$out_file" "$staging"
    else
        local out_file="$name.tar.gz"
        tar czf "$out_dir/$out_file" --directory="$staging" .
    fi
}

main() {
    cargo test 
    && cargo build --target "$TARGET" --release 
    && package_executable
}

main