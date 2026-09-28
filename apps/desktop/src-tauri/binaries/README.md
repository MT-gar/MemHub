Place the `memhub` CLI here before `tauri build`, named with the target triple, e.g.

    memhub-x86_64-apple-darwin
    memhub-aarch64-apple-darwin
    memhub-x86_64-pc-windows-msvc.exe
    memhub-x86_64-unknown-linux-gnu

`scripts/build-sidecar.sh` does this for the host platform; the release workflow does it per target.
