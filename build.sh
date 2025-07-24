#!/bin/bash
pnpm run build
rustc --print target-list
export RUSTUP_DIST_SERVER=https://mirrors.aliyun.com/rustup
rustup target add x86_64-pc-windows-msvc
rustup target add x86_64-apple-darwin
pnpm tauri build --runner cargo-xwin --target x86_64-apple-darwin
pnpm tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc
