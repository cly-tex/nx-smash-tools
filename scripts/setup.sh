#!/bin/bash

# Check if the setup files directory exists in the expected location, otherwise this is not being run from the root
if [ ! -d "scripts/setup-files" ]; then
    echo "scripts/setup-files directory does not exist relative to the current directory, are you running scripts/setup.sh from the repository root?"
    exit 2
fi

# Check that the user has Rustup, otherwise redirect them to install it
if ! command -v rustup >/dev/null 2>&1; then
    echo "rustup not found, make sure that you have installed Rust: https://rustup.rs/"
    exit 1
fi

# Add nightly if they don't have it then add the base target triple for switch development
rustup toolchain add nightly
rustup target add aarch64-unknown-none

# Check if linke is installed, otherwise install it so that we can produce NRO and NSO files
if ! command -v linkle >/dev/null 2>&1; then
    echo "Installing Linkle"
    cargo install --features=binaries linkle
fi

# Init and copy the build files
mkdir -p .build-files
cp scripts/setup-files/link.ld .build-files/link.ld
cp scripts/setup-files/template-target.json .build-files/switch.json

# Init and copy the LSP config
mkdir -p .zed
cp scripts/setup-files/template-zed-settings.json .zed/settings.json

# Replace the uses of {{PROJECT_DIR}} with the current directory
sed -i '' -e "s#{{PROJECT_DIR}}#$(pwd)#g" .build-files/switch.json
sed -i '' -e "s#{{PROJECT_DIR}}#$(pwd)#g" .zed/settings.json
