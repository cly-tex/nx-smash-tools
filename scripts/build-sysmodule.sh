#!/bin/bash

set -e

if [ ! -d "binaries" ]; then
    echo "Run this script from the root of the repository only"
    exit
fi

if [ "$#" -ne 1 ]; then
    echo "Pass only the name of the sysmodule that you want to build"
    exit
fi

if [ ! -d "binaries/$1" ]; then
    echo "Binary '$1' does not exist"
    exit
fi

if ! command -v npdmtool >/dev/null 2>&1; then
    echo "npdmtool not found, is devkitpro installed?"
    exit
fi

if ! command -v build_pfs0 >/dev/null 2>&1; then
    echo "build_pfs0 not found, is devkitpro installed?"
    exit
fi

if ! command -v rustup >/dev/null 2>&1; then
    echo "rustup nout found, make sure to install Rust: https://rustup.rs/"
    exit
fi

bin_name="${1//-/_}"
rustup run nightly cargo build -Zjson-target-spec --target .build-files/switch.json -Zbuild-std=core,panic_abort,alloc --release -p $1
linkle nso target/switch/release/lib${bin_name}.so target/switch/release/lib${bin_name}.nso
npdmtool binaries/${1}/npdm.json target/switch/release/${bin_name}.npdm
mkdir -p target/switch/release/exefs_${bin_name}
cp target/switch/release/lib${bin_name}.nso target/switch/release/exefs_${bin_name}/main
cp target/switch/release/${bin_name}.npdm target/switch/release/exefs_${bin_name}/main.npdm
build_pfs0 target/switch/release/exefs_${bin_name} target/switch/release/${bin_name}.nsp
