# 构建 / Build

这是 ARM64 Switch NRO 项目，需要配置可用于 Switch 的 Rust 标准库与 Skyline 工具链。当前发行基于 `nightly-2026-02-14` 与匹配的定制 Switch 标准库；原构建工具链别名为 `cobalt-zip-audio`。普通桌面 Rust target 不能替代 Switch 标准库。

This ARM64 Switch NRO requires a configured Switch Rust standard library and Skyline toolchain. The current release was built using `nightly-2026-02-14` with a matching customized Switch standard library, under the local alias `cobalt-zip-audio`. A normal desktop Rust target is insufficient.

工具链配置参考 [Skyline Rust](https://github.com/skyline-rs/rust-src)。当前源树包含本地依赖快照、Cargo.lock、目标 JSON、链接脚本和 ELF → NRO 转换器源码，不包含编译器或本机 Cargo 缓存。Switch 标准库相关版本信息见下方。

The source includes local dependency snapshots, Cargo.lock, target JSON, linker script and ELF-to-NRO converter source. It does not bundle a compiler or the original Cargo download cache. Toolchain reference: [Skyline Rust](https://github.com/skyline-rs/rust-src).

```powershell
python tools/build.py --toolchain cobalt-zip-audio
```

首次可能需要联网下载 Cargo.lock 中的依赖。依赖已缓存时可添加 `--offline`。工具链别名可替换为自己已配置的兼容工具链。脚本仅在本项目中生成 `target/` 和 `dist/`，不安装或修改全局工具链。

The first build may download dependencies locked in Cargo.lock. Add `--offline` when dependencies are cached. Substitute your configured compatible toolchain alias as needed. The script writes project-local `target/` and `dist/` only; it does not install or change global toolchains.

## 当前发行构建基线 / Current release baseline

- Rust compiler/source baseline: `nightly-2026-02-14`, matching Rust source commit `fabcf8e5bebc0d31e33717778a89df97d2b0e443`.
- Switch std libc baseline: `12c40129992aa01e554a5355612df4a58f06b53b`; the original local std used four dependency-location adjustments, without implementation changes.
- `tools/rustc-wrapper.rs` supplies `-Zunstable-options` for the custom target.
- `tools/build.py` resolves the linker script relative to this project at build time.
- Generated `dist/*.nro` is a new build, and need not be byte-identical to `Releases/*.nro` because package names, paths or compiler metadata can differ.
- `Releases/*.nro` is the existing compiled binary copied unchanged from the current integration package. The prepared source was rebuilt successfully for verification in dist/, without replacing the original release binary or claiming new game validation.

## 主机工具编译器 / Host compiler

ELF 转 NRO 工具的锁定依赖要求比旧版 stable 更新的 Rust。本脚本默认使用 `nightly-2026-02-14` 编译主机工具；可通过 `--host-toolchain` 指定已安装的兼容 Rust，不自动升级本机 stable。

Locked converter dependencies require a Rust compiler newer than some old stable installations. The script defaults to `nightly-2026-02-14` for host tools; override with `--host-toolchain` using an installed compatible compiler. It does not upgrade the machine's stable toolchain.
