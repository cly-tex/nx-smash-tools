# Bootstrapping
To begin with developing, make sure that you have [Rust installed](https://rustup.rs/).

Afterwards, run the setup script from the root of the repository:
```
./scripts/setup.sh
```

On Linux/macOS, this should create:
- `.build-files/nso.ld` - The linkerscript required to build NSO modules
- `.build-files/nro.ld` - The linkerscript required to build NRO modules
- `.build-files/switch.json` - The LLVM target defintion for the Switch
- `.zed/settings.json` - LSP settings for rust-analyzer so that it can check/lint the code in this project. It defaults to using `clippy`

If you want to use Windows, you are on your own for the setup because I do not have a Windows machine to author scripts for.
