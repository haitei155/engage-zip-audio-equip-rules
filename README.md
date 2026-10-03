# Fire Emblem Engage — ZIP 语音与纹章士装备规则

[English](README.en.md)

首发版本：**[v0.1](https://github.com/haitei155/engage-zip-audio-equip-rules/releases/tag/v0.1)**。

**必须先安装 [Cobalt](https://github.com/Raytwo/Cobalt)。** 感谢 **Raytwo** 开发并维护 Cobalt，为本项目提供 MOD 加载与扩展基础。

当前编译版面向《火焰纹章 Engage》2.0.0，使用 Cobalt 1.31.0 作为兼容基线。

## 功能

- 为 MOD ZIP 内的 BNK/WEM 声音文件生成经校验的 SD 缓存，重定向游戏文件读取。无需手工解压角色声音。压缩包内声音更新后，下次完整启动游戏会自动读取新内容、识别变化并生成经校验的新缓存，无需手工清理缓存。
- 读取各 MOD 的 `patches/god_equip_restrictions/*.txt`，限制可用普通人物装备自己的同名纹章士。
- 按配置覆盖双人纹章士和三级长共用形态；不改变其他人物的原生装备判定。
- 不补充缺失录音，不自动卸下存档中已经装备的戒指或手镯。

## 同名纹章士装备限制示例

对自定义新增角色配置同名纹章士配对规则后，该角色无法装备对应纹章士的戒指／手镯。下图中，可用普通人物艾莉可对应的纹章士艾莉可条目已被禁用；其他条目继续保留正常选择。

![艾莉可无法装备自己的同名纹章士手镯](images/same-character-emblem-equipment-restriction.jpg)

该机制适用于今后新增的角色，但角色 MOD 需要提供对应的 PID/GID 规则；当前实现不会只比较显示名称就自动限制。配置方法见 [装备规则说明](docs/CONFIGURATION.md)。图中展示的是装备限制，ZIP 语音缓存功能另见上方说明。

## 为新增角色添加配对规则（无需修改源码）

只要已安装本插件和定义对应人物／纹章士的 MOD，新增限制仅需添加规则文件，**不用修改源码或重新编译 NRO**。插件按实际 PID/GID 匹配，而非按角色显示名称匹配。

1. 在角色 MOD 的根目录建立 `patches/god_equip_restrictions/my_character.txt`。若使用 ZIP，文件在 ZIP 内的路径也必须从 `patches/` 开始。
2. 使用 UTF-8 编码，每行填写一个人物 PID 和一个纹章士 GID，用空格或制表符分隔。下面是当前谢兹包的实际规则：

```text
# 普通人物谢兹不能装备纹章士谢兹
PID_MOD_SHEZ GID_Shez
```

新增自己的角色时，可按以下模板填写；`PID_MOD_MY_CHARACTER`、`GID_MY_EMBLEM` 是占位示例，请替换为人物定义（如 `Person.xml`）和纹章士定义（如 `God.xml`）中真实、大小写一致的 ID：

```text
PID_MOD_MY_CHARACTER GID_MY_EMBLEM
```

3. 如果要禁止同一人物装备多个对应形态，每个实际 GID 单独写一行；双人纹章士、三级长等共用形态也需明确列出，插件不会根据名称自动展开。
4. 保存并安装更新后的角色 MOD，完整退出并重新启动游戏，随后进入戒指／手镯选择界面核对禁用状态。规则首次读取后会缓存，返回标题界面不能保证重新读取。

所有已加载 MOD 的规则会合并，因此未来追加角色可以在自己的独立包内提供规则。空行和以 `#` 开头的整行注释会被忽略；不要在配对行末尾追加注释。已有原版装备限制继续生效；本规则只增加禁用配对，不会创建人物、纹章士，也不会自动卸下存档中已装备的戒指／手镯。

## 下载与安装

当前编译版：[`engage_zip_audio_equip_rules.nro`](Releases/engage_zip_audio_equip_rules.nro)；校验值见 [SHA256SUMS](Releases/SHA256SUMS)。

1. 先按 Cobalt 的说明安装基础运行环境。
2. 将 NRO 放在 `engage/mods/engage-zip-audio-equip-rules/` 下，完整重启游戏。
3. 如果 12 人整合包已包含本插件，就使用包内版本，避免再装一个独立副本。

语音缓存位于 `sd:/engage/.cache/wwise-zip-audio`。缓存标识包含完整声音路径（含语言）、长度和内容；启动时还会逐字节检查已有缓存，损坏或未完成写入的缓存会重建。此机制在游戏启动时执行，不支持运行中替换 ZIP 后立即热更新。完整退出后再启动游戏，才能重新索引已更新的角色包声音。装备规则的格式与例子见 [配置说明](docs/CONFIGURATION.md)。

## 源码与构建

见 [构建说明](docs/BUILD.md)。源码包括当前功能实现和实际使用的本地依赖快照；构建产物输出到 `dist/`，不会覆盖 `Releases/` 中的发行编译版。

## 许可与致谢

本项目自行新增的代码与文档采用 [Apache License 2.0](LICENSE)。第三方依赖、上游派生代码及资源保留各自许可与权利声明；详细来源与范围见 [THIRD_PARTY.md](THIRD_PARTY.md) 和 [NOTICE](NOTICE)。

感谢 [Raytwo](https://github.com/Raytwo/Cobalt)、[DivineDragonFanClub](https://github.com/DivineDragonFanClub/engage-il2cpp)、[skyline-rs](https://github.com/skyline-rs) 以及相关依赖贡献者。
