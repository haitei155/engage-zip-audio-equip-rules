# v0.1 — ZIP 语音与纹章士装备规则 / ZIP Audio and Emblem Equipment Rules

自动生成并校验 ZIP 内 BNK/WEM 声音缓存，路径为 sd:/engage/.cache/wwise-zip-audio。声音更新在下次完整启动时自动识别，无需手工解压或清缓存。支持 MOD 提供 UTF-8 PID/GID 规则禁止角色装备对应同名纹章士。

Verified caching of BNK/WEM audio inside mod ZIPs at sd:/engage/.cache/wwise-zip-audio. Audio updates are detected on the next full launch, without manual extraction or cache clearing. Mod-supplied UTF-8 PID/GID rules prevent configured characters from equipping matching emblems.

## 安装 / Installation

先安装 [Cobalt](https://github.com/Raytwo/Cobalt)。下载 `engage_zip_audio_equip_rules.nro`，放入 `engage/mods/engage-zip-audio-equip-rules/`，完整重启游戏。如果整合包已包含本插件，使用包内版本，避免重复加载。

Install Cobalt first. Place `engage_zip_audio_equip_rules.nro` in `engage/mods/engage-zip-audio-equip-rules/` and fully restart the game. Use the included copy when an integration package already provides the plugin.

基线 / Baseline: Fire Emblem Engage 2.0.0 / Cobalt 1.31.0.

## 验证 / Validation

已为无版本后缀缓存路径重新编译。Switch 编译、NRO 转换及缓存内容变化/损坏重建宿主验证通过；本轮路径调整仍待游戏复测。 / Rebuilt for the unversioned cache path. Switch compilation, NRO conversion and host cache-refresh/repair checks passed; the path revision awaits an in-game retest.

SHA-256: `7d6596f598004c66ed66b1effa238215a42d7f7cdda58b0d3daf00f186d227f7`；下载资产含 `SHA256SUMS`。

许可、来源与致谢见仓库 LICENSE、NOTICE、THIRD_PARTY.md 和 README。
See LICENSE, NOTICE, THIRD_PARTY.md and README for licensing, origins and credits.
