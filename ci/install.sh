#!/usr/bin/env sh

case "$TRAVIS_OS_NAME" in 
    "windows")
        choco install ruby make upx 7zip
        ;;
    "macos")
        brew install ruby make upx
        ;;
    *)
        apt-get install ruby make upx
        ;;
esac
