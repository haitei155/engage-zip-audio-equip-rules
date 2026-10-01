# 装备规则 / Equipment Rules

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

## Adding Rules for New Characters (No Source Changes)

With this plugin and the mods defining the corresponding person and emblem installed, add a rule file to introduce a new restriction. **No source modification or NRO rebuild is required.** Matching uses the actual PID/GID values, rather than character display names.

1. Create `patches/god_equip_restrictions/my_character.txt` at the character mod root. For a ZIP mod, the path inside the archive must also start with `patches/`.
2. Save it as UTF-8. Each line contains one person PID and one emblem GID, separated by spaces or tabs. This is the actual rule used by the current Shez package:

```text
# Playable Shez cannot equip emblem Shez
PID_MOD_SHEZ GID_Shez
```

For your own added character, use this template. `PID_MOD_MY_CHARACTER` and `GID_MY_EMBLEM` are placeholders; replace them with the exact, case-sensitive IDs in the person definitions (such as `Person.xml`) and emblem definitions (such as `God.xml`):

```text
PID_MOD_MY_CHARACTER GID_MY_EMBLEM
```

3. To block several corresponding forms for the same person, list each actual GID on its own line. Paired emblems and house-leader forms also require explicit entries; the plugin does not expand them by name.
4. Install the updated character mod, fully exit and restart the game, then check the disabled entry on the ring/bracelet selection screen. Rules are cached after their first read; returning to the title screen does not guarantee a reload.

Rules from all loaded mods are combined, so future characters can supply them in their own standalone packages. Blank lines and full-line `#` comments are ignored; do not append comments to pair lines. Native restrictions remain effective. These rules only add denied pairs: they do not define persons or emblems, or forcibly unequip rings/bracelets already stored in a save.

