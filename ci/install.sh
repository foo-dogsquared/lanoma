#!/bin/bash

set -ex

. "$(dirname $0)/utils.sh"

install_rustup() {
    case "$TRAVIS_OS_NAME" in
        windows)
            curl -sSf -o rustup-init.exe https://win.rustup.rs/ 
            rustup-init.exe -y --default-host "$TARGET" 
            ;;
        *)
            curl https://sh.rustup.rs -sSf \
            | sh -s -- -y --default-toolchain="$TRAVIS_RUST_VERSION"
            ;;
    esac
    rustc -V
    cargo -V
}

install_target() {
    if [ $(host) != "$TARGET" ]; then
        rustup target add "$TARGET"
    fi
}

# Installing the dependencies 
case "$TRAVIS_OS_NAME" in 
    "windows")
        choco install ruby make upx 7zip
        ;;
    "macos")
        sudo brew install ruby make upx 7zip 
        ;;
    *)
        sudo add-apt-repository universe
        sudo apt-get update
        sudo apt-get install ruby make upx-ucl p7zip-full build-essential 
        ;;
esac

main() {
    install_target
}

main