#!/bin/bash

# Originally from https://github.com/latex3/latex3

# This script is used for building LaTeX files using Travis
# A minimal current TL is installed adding only the packages that are
# required

trap 'Error on line $LINENO' ERR

# See if there is a cached version of TL available
if ! command -v texlua > /dev/null; then
  # Obtain TeX Live
  wget http://mirror.ctan.org/systems/texlive/tlnet/install-tl.zip
  7z x install-tl.zip

  # Install a minimal system
  # This is executed from `install.sh` so the path is executed relative to it. 
  if [ $TRAVIS_OS_NAME = "linux" ]; then
    ./install-tl-20*/install-tl --profile="$(dirname $0)/texlive/texlive.profile"
  else
    ./install-tl-20*/install-tl-windows.bat -profile="$(dirname $0)/texlive/texlive.profile.windows"
  fi
fi

ls --recursive $HOME
