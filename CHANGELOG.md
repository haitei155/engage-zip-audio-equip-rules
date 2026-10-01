# 更新记录 / Changelog

## v0.1 — 2026-10-02

自动生成并校验 ZIP 内 BNK/WEM 声音缓存，路径为 sd:/engage/.cache/wwise-zip-audio。声音更新在下次完整启动时自动识别，无需手工解压或清缓存。支持 MOD 提供 UTF-8 PID/GID 规则禁止角色装备对应同名纹章士。

Verified caching of BNK/WEM audio inside mod ZIPs at sd:/engage/.cache/wwise-zip-audio. Audio updates are detected on the next full launch, without manual extraction or cache clearing. Mod-supplied UTF-8 PID/GID rules prevent configured characters from equipping matching emblems.

已为无版本后缀缓存路径重新编译。Switch 编译、NRO 转换及缓存内容变化/损坏重建宿主验证通过；本轮路径调整仍待游戏复测。 / Rebuilt for the unversioned cache path. Switch compilation, NRO conversion and host cache-refresh/repair checks passed; the path revision awaits an in-game retest.
