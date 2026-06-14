#!/bin/bash

set -e

if [ ! -d "binaries" ]; then
    echo "Run this script from the root of the repository only"
    exit
fi

if [ "$#" -ne 2 ]; then
    echo "Usage: scripts/build-module.sh <binary> <nro|nso>"
    exit
fi

if [ "$2" != "nso" ]; then
    if [ "$2" != "nro" ]; then
        echo "Usage: scripts/build-module.sh <binary> <nro|nso>"
        exit
    fi
fi

if [ ! -d "binaries/$1" ]; then
    echo "Binary '$1' does not exist"
    exit
fi

if ! command -v rustup >/dev/null 2>&1; then
    echo "rustup nout found, make sure to install Rust: https://rustup.rs/"
    exit
fi

rustup run nightly cargo build -Zjson-target-spec --target .build-files/switch.json -Zbuild-std=core,panic_abort,alloc --release -p $1

bin_name="${1//-/_}"

linkle ${2} target/switch/release/lib${bin_name}.so target/switch/release/lib${bin_name}.${2}
