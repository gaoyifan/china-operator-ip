# Issue #45：未宣告中国地址的自动登记兜底

## 目标

在保留 BGP 优先语义的前提下，让 `china` 自动收录全球 RIR 登记为中国、但当前没有具体 BGP 宣告的 IPv4 和 IPv6 地址，而不是逐条维护人工 CIDR。

Issue #45 的 `121.46.0.0/18` 是这一类问题的实例：登记范围属于中国，但只有其中一部分存在可观测路由。解决方案应覆盖同类地址，并随 RIR 登记和每日 RIB 自动变化。

登记国家只能说明资源持有组织的注册国家，不能证明地址当前由中国电信、移动或其他具体运营商承载。因此登记兜底只进入 `china`，具体运营商列表仍完全由 BGP 分类产生。

## 数据源

生成流程下载五个 RIR 的 NRO extended delegated statistics：

- AFRINIC
- APNIC
- ARIN
- LACNIC
- RIPE NCC

只选取国家码为 `CN`、类型为 `ipv4` 或 `ipv6`、状态为 `allocated` 或 `assigned` 的记录。IPv4 记录是起始地址和地址数量，可能不是单个 CIDR；生成器将其转换为最小 CIDR 集合。IPv6 的 `value` 是前缀长度。

扫描全部 RIR 可以覆盖由 APNIC 之外的 RIR 分配给中国组织的资源以及未来的跨 RIR 转移。`available` 和 `reserved` 记录没有分配给资源持有者，不进入候选集合。

## 结果语义

定义：

- `C`：按现有 ASN、AS_PATH 和可信国内 transit 规则得到的 BGP 中国分类结果；
- `F`：五个 RIR delegated 数据中符合上述条件的 CN 登记空间；
- `A`：输入的所有 RIB 中出现过的全部非默认宣告前缀，不受目标 ASN、国家、私有 ASN 或可信 transit 过滤影响。IPv4/IPv6 默认路由不代表每个地址块都有全球具体路由，因此不计入 `A`。

最终结果为：

```text
china = simplify(C ∪ (F − A))
```

这使登记数据始终低于 BGP：

- 未宣告空间由 RIR 登记兜底补齐；
- 任意 ASN 一旦宣告其中一部分，该部分立即由 BGP 分类决定；
- 境外 ASN 宣告的前缀不会被 CN 登记覆盖；
- 路由撤回后，仍登记为 CN 的空间会自动回到兜底；
- RIR 登记增加、转移或删除后，下一次生成自动反映变化。

## 模块与接口

`operators.yaml` 只声明策略：

```yaml
china:
  registry_fallback_country: 'CN'
```

生成器拥有 RIR 外部格式的实现，负责下载五个 delegated 文件、筛选记录、转换非 CIDR IPv4 范围，并生成可审计的 `result/.china.registered.txt`。

`bgptools` 不理解 RIR 格式，只通过一个稳定的通用接口读取规范 CIDR：

```text
--fallback-prefix-file result/.china.registered.txt
```

它在原有目标 ASN 结果生成后计算 `F − A`。完整非默认宣告集合随 ASN 地址范围一起缓存；fallback 文件在读取缓存后应用，因此其路径和内容不参与缓存键，登记数据更新不需要重新解析 RIB。

## 非目标

- 不使用 WHOIS inetnum 描述、商业地理库或 IP 探测推断位置；delegated 国家码是本兜底唯一的登记依据。
- 不把登记资源映射为电信、移动、联通等具体运营商。
- 不覆盖或纠正任何已宣告前缀的 BGP 分类。
- 不新增一套面向用户的 allocated 地址产物；规范化登记输入只作为隐藏审计文件随构建发布。

## 验证

`bgptools` 行为测试覆盖：

1. 已分类半段与未宣告半段能合并为完整 fallback；
2. fallback 内已宣告的更具体前缀保持为空洞；
3. fallback 完全被宣告覆盖时不增加地址；
4. IPv4 和 IPv6 使用相同规则；
5. CLI 接受 fallback CIDR 文件。

仓库集成验证覆盖：

- 五个 RIR 文件均被读取，非 CIDR IPv4 范围被无损转换；
- 只包含 CN 的 `allocated`、`assigned` IPv4/IPv6 记录；
- `just gen china` 传入规范化登记文件；其他运营商不传入；
- 固定 RIB 下，其他运营商输出逐字节不变；
- `china` 的集合差仅包含登记为 CN 且未宣告的地址；
- Issue #45 的结果包含 `121.46.0.0/18`，具体运营商集合不因此扩张。

## 发布顺序

该改动跨越 `bgptools` 和本仓库，按以下顺序发布：

1. 合并并发布 `bgptools 0.3.4`；
2. 本仓库固定并使用 `0.3.4`，同时启用 RIR 数据准备和文件透传；
3. 在 pull request 构建中检查完整集合差和隐藏登记输入；
4. 合并后手动触发一次 `update_ip_lists`，检查产物再恢复每日自动更新。
