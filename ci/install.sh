#!/usr/bin/env sh

case "$TRAVIS_OS_NAME" in 
    "windows")
        choco install ruby make upx 7zip
        ;;
    "macos")
        sudo brew install ruby make upx
        ;;
    *)
        sudo apt-get install ruby make upx
        ;;
esac
