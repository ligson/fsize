#!/bin/bash
pnpm run build
rustc --print target-list
export RUSTUP_DIST_SERVER=https://mirrors.aliyun.com/rustup
rustup target add x86_64-pc-windows-msvc
tauri build --target x86_64-apple-darwin
tauri build --target x86_64-pc-windows-msvc
