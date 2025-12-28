<!-- Keep these links. Translations will automatically update with the README. -->
[中文](https://zdoc.app/zh/gaoyifan/china-operator-ip) | 
[Deutsch](https://zdoc.app/de/gaoyifan/china-operator-ip) | 
[English](https://zdoc.app/en/gaoyifan/china-operator-ip) | 
[Español](https://zdoc.app/es/gaoyifan/china-operator-ip) | 
[français](https://zdoc.app/fr/gaoyifan/china-operator-ip) | 
[日本語](https://zdoc.app/ja/gaoyifan/china-operator-ip) | 
[한국어](https://zdoc.app/ko/gaoyifan/china-operator-ip) | 
[Português](https://zdoc.app/pt/gaoyifan/china-operator-ip) | 
[Русский](https://zdoc.app/ru/gaoyifan/china-operator-ip)

# 中国运营商IP地址库

依据中国网络运营商分类的IP地址库

## 为什么要创建这个项目

国内在BGP/ASN数据分析和应用方面，目前主要有[ipip.net](https://www.ipip.net)等商业服务，其运营商IP库的准确度较高。

随着互联网的持续发展，边界网关协议（BGP）成为处理大规模路由数据不可或缺的基础协议之一。通过BGP，新的IP地址（或前缀）可以在全球互联网上对外通告，并被其他自治系统学习和访问。因此，BGP数据为分析归属和运营商IP分类提供了宝贵的数据基础。

不过，目前国内大部分IP库依赖[WHOIS数据库](https://ftp.apnic.net/apnic/whois/apnic.db.inetnum.gz)作为数据源。WHOIS虽然能标明IP的注册机构，但无法体现实际使用情况，这会导致一些并非运营商亲自宣告的IP地址被分类不准确。像ipip.net这样较早开始结合BGP与ASN数据进行分析的公司，能够提供较为丰富和准确的数据服务，但其高质量数据部分需要付费。

在其他项目中我曾用到BGP数据，因此基于开源的想法整理和公布了这些相关代码，形成了本项目。该IP库可以灵活应用于多种场景，例如[@ustclug](https://github.com/ustclug)利用它在权威DNS服务器中进行分域解析，或者作为按运营商出口分流的参考等。

受个人精力限制，本项目的IP覆盖率难以与商业服务商持平，特别是在部分骨干节点相关的地址上，可能会有遗漏，但这些情况一般对大多数用户影响较小。

如有建议或问题，欢迎通过[issue](https://github.com/gaoyifan/china-operator-ip/issues)反馈。

## 收录的运营商

* 中国电信(chinanet)
* 中国移动(cmcc)
* 中国联通(unicom)
* ~~中国铁通(tietong)~~<已废弃>
* 教育网(cernet)
* 科技网(cstnet)
* 鹏博士(drpeng) <试验阶段>
* 谷歌中国(googlecn) <试验阶段>

*P.S. 由于移动与铁通已合并，铁通集合已废弃，详见[issue #10](https://github.com/gaoyifan/china-operator-ip/issues/10)。*

*P.S. 鹏博士集团（包括：鹏博士数据、北京电信通、长城宽带、宽带通）的IP地址并非全都由独立的自治域做宣告，目前大部分地址仍由电信、联通、科技网代为宣告。故[列表](https://github.com/gaoyifan/china-operator-ip/blob/ip-lists/drpeng.txt)中的地址仅为鹏博士拥有的部分IP地址，且这些IP同时具有电信、联通两个上级出口。详见[issue #2](https://github.com/gaoyifan/china-operator-ip/issues/2).*

*P.S. 如果需要国内所有地址的集合，请参考 [chnroutes2](https://github.com/misakaio/chnroutes2) 项目*

## 如何获取数据

### 方法1：使用预生成结果

IP列表（CIDR格式）保存在仓库的[ip-lists分支](https://github.com/gaoyifan/china-operator-ip/tree/ip-lists)中，GitHub Actions每日自动更新。

```sh
git clone -b ip-lists https://github.com/gaoyifan/china-operator-ip.git
```

亦可通过以下站点获取：

| 运营商 | [EdgeOne Pages](https://china-operator-ip.yfgao.com) | [GitHub Pages](https://gaoyifan.github.io/china-operator-ip) |
|---|---|---|
| 中国 | [IPv4](https://china-operator-ip.yfgao.com/china.txt) \| [IPv6](https://china-operator-ip.yfgao.com/china6.txt) \| [IPv4+IPv6](https://china-operator-ip.yfgao.com/china46.txt) | [IPv4](https://gaoyifan.github.io/china-operator-ip/china.txt) \| [IPv6](https://gaoyifan.github.io/china-operator-ip/china6.txt) \| [IPv4+IPv6](https://gaoyifan.github.io/china-operator-ip/china46.txt) |
| 中国电信 | [IPv4](https://china-operator-ip.yfgao.com/chinanet.txt) \| [IPv6](https://china-operator-ip.yfgao.com/chinanet6.txt) \| [IPv4+IPv6](https://china-operator-ip.yfgao.com/chinanet46.txt) | [IPv4](https://gaoyifan.github.io/china-operator-ip/chinanet.txt) \| [IPv6](https://gaoyifan.github.io/china-operator-ip/chinanet6.txt) \| [IPv4+IPv6](https://gaoyifan.github.io/china-operator-ip/chinanet46.txt) |
| 中国移动 | [IPv4](https://china-operator-ip.yfgao.com/cmcc.txt) \| [IPv6](https://china-operator-ip.yfgao.com/cmcc6.txt) \| [IPv4+IPv6](https://china-operator-ip.yfgao.com/cmcc46.txt) | [IPv4](https://gaoyifan.github.io/china-operator-ip/cmcc.txt) \| [IPv6](https://gaoyifan.github.io/china-operator-ip/cmcc6.txt) \| [IPv4+IPv6](https://gaoyifan.github.io/china-operator-ip/cmcc46.txt) |
| 中国联通 | [IPv4](https://china-operator-ip.yfgao.com/unicom.txt) \| [IPv6](https://china-operator-ip.yfgao.com/unicom6.txt) \| [IPv4+IPv6](https://china-operator-ip.yfgao.com/unicom46.txt) | [IPv4](https://gaoyifan.github.io/china-operator-ip/unicom.txt) \| [IPv6](https://gaoyifan.github.io/china-operator-ip/unicom6.txt) \| [IPv4+IPv6](https://gaoyifan.github.io/china-operator-ip/unicom46.txt) |
| 教育网 | [IPv4](https://china-operator-ip.yfgao.com/cernet.txt) \| [IPv6](https://china-operator-ip.yfgao.com/cernet6.txt) \| [IPv4+IPv6](https://china-operator-ip.yfgao.com/cernet46.txt) | [IPv4](https://gaoyifan.github.io/china-operator-ip/cernet.txt) \| [IPv6](https://gaoyifan.github.io/china-operator-ip/cernet6.txt) \| [IPv4+IPv6](https://gaoyifan.github.io/china-operator-ip/cernet46.txt) |
| 科技网 | [IPv4](https://china-operator-ip.yfgao.com/cstnet.txt) \| [IPv6](https://china-operator-ip.yfgao.com/cstnet6.txt) \| [IPv4+IPv6](https://china-operator-ip.yfgao.com/cstnet46.txt) | [IPv4](https://gaoyifan.github.io/china-operator-ip/cstnet.txt) \| [IPv6](https://gaoyifan.github.io/china-operator-ip/cstnet6.txt) \| [IPv4+IPv6](https://gaoyifan.github.io/china-operator-ip/cstnet46.txt) |
| 鹏博士 | [IPv4](https://china-operator-ip.yfgao.com/drpeng.txt) \| [IPv6](https://china-operator-ip.yfgao.com/drpeng6.txt) \| [IPv4+IPv6](https://china-operator-ip.yfgao.com/drpeng46.txt) | [IPv4](https://gaoyifan.github.io/china-operator-ip/drpeng.txt) \| [IPv6](https://gaoyifan.github.io/china-operator-ip/drpeng6.txt) \| [IPv4+IPv6](https://gaoyifan.github.io/china-operator-ip/drpeng46.txt) |
| 谷歌中国 | [IPv4](https://china-operator-ip.yfgao.com/googlecn.txt) \| [IPv6](https://china-operator-ip.yfgao.com/googlecn6.txt) \| [IPv4+IPv6](https://china-operator-ip.yfgao.com/googlecn46.txt) | [IPv4](https://gaoyifan.github.io/china-operator-ip/googlecn.txt) \| [IPv6](https://gaoyifan.github.io/china-operator-ip/googlecn6.txt) \| [IPv4+IPv6](https://gaoyifan.github.io/china-operator-ip/googlecn46.txt) |
| 统计 | [stat](https://china-operator-ip.yfgao.com/stat) | [stat](https://gaoyifan.github.io/china-operator-ip/stat) |

镜像说明：
* **EdgeOne Pages**: 中国大陆境内完整镜像
* **GitHub Pages**: 海外完整镜像 

### 方法2：从BGP数据生成

#### 安装依赖

* [just](https://github.com/casey/just?tab=readme-ov-file#installation)
* [Rust Toolchain](https://www.rust-lang.org/tools/install)
* [bgpkit-broker](https://github.com/bgpkit/bgpkit-broker) (`cargo install bgpkit-broker@0.7.0`)
* [bgptools](https://github.com/gaoyifan/bgptools) (`cargo install bgptools@0.3.0`)
* [aria2](https://github.com/aria2/aria2)
* [Ruby](https://www.ruby-lang.org)

#### 生成IP列表

```shell
just
```

注：执行 `just --list` 查看所有可用的命令。

## 社区关联项目
- [OneOhCloud/One-GeoIP](https://github.com/OneOhCloud/one-geoip): 适用于 sing-box 的规则集
- [fcshark-org/route-list](https://github.com/fcshark-org/route-list): 适用于 dnsmasq 的规则集
- [zxlhhyccc/smartdns-list-scripts](https://github.com/zxlhhyccc/smartdns-list-scripts): smartdns 使用的规则集

## Acknowledgments

* 感谢[boj](https://ring0.me)师兄提出的[设计思路](https://github.com/ustclug/discussions/issues/79#issuecomment-267958775)
* [bgpkit](https://bgpkit.com)
* [University of Oregon Route Views Archive Project](http://archive.routeviews.org)
* [GitHub Action](https://github.com/features/actions)
* [Tencent EdgeOne](https://edgeone.ai/zh?from=github)

## License

[MIT License](LICENSE)
