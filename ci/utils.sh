#!/bin/bash

host() {
    case "$TRAVIS_OS_NAME" in
        linux)
            echo x86_64-unknown-linux-gnu
            ;;
        osx)
            echo x86_64-apple-darwin
            ;;
        windows)
            echo x86_64-pc-windows
    esac
}

tempdir() {
    case "$TRAVIS_OS_NAME" in 
        windows)
            echo C:\\\\Windows\\Temp\\
            ;;
        *)
            echo /tmp/
            ;;
    esac
}
