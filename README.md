# 中国运营商IP地址库
[![FOSSA Status](https://app.fossa.io/api/projects/git%2Bgithub.com%2Fgaoyifan%2Fchina-operator-ip.svg?type=shield)](https://app.fossa.io/projects/git%2Bgithub.com%2Fgaoyifan%2Fchina-operator-ip?ref=badge_shield)


依据中国网络运营商分类的IP地址库

## 为什么创造这个项目

在国内，BGP/ASN数据分析的商业服务只有一个[ipip.net](https://www.ipip.net)，是目前运营商IP库准确度最高的服务商，我认为没有之一。

随着互联网规模的增加，为了处理大批量的路由数据，边界网关协议（即BGP，下同）应运而生，是互联网的基础协议之一。为了保证了全球网络路由的可达性，但凡需要在互联网中注册一个IP（段），都需要借助BGP协议对外宣告，这样互联网中的其他自治域才能学习到这段地址的路由信息，其它主机才能成功访问这个IP（段）。因此可以说，BGP数据是最适合分析运营商IP地址的数据来源之一。

但是，目前国内绝大多数IP库都由[WHOIS数据库](https://ftp.apnic.net/apnic/whois/apnic.db.inetnum.gz)作为基础数据来源。WHOIS数据仅表示某个IP被哪个机构注册，但无从知晓该IP被用在何处，这就导致许多非运营商自己注册的IP地址无法被正确分类。ipip.net是最早开始做BGP/ASN数据分析的公司之一，数据准确性甩其它库几条街。但很可惜是，ipip.net作为商业公司，绝大多数高质量的IP数据都是收费的，且价格不菲。

由于在做其他课题时需要处理BGP数据，本着开源精神，我将这部分代码重新封装，创造了这个项目。至于如何使用，大家可以自己发挥想象力。如：[@ustclug](https://github.com/ustclug)将其用在权威DNS服务器上做分域解析；我则借助这个IP库做了一个多出口的网关，访问不同的运营商时走不同的线路（如果都不匹配则走国外vps，原因你懂的）。

但由于个人精力有限，IP库的覆盖率并不及ipip.net，尤其是一些骨干网节点的地址，这些地址往往是核心路由设备或企业托管给运营商的地址，对普通用户影响不大。

如果大家有任何建议或疑问，欢迎提交issue。

## 收录的运营商

* 中国电信(chinanet)
* 中国移动(cmcc)
* 中国联通(unicom)
* ~~中国铁通(tietong)~~<即将废弃>
* 教育网(cernet)
* 科技网(cstnet)
* 鹏博士(drpeng) <试验阶段>
* 谷歌中国(googlecn) <试验阶段>

*P.S. 由于移动与铁通已合并，铁通集合即将废弃，详见[issue #10](https://github.com/gaoyifan/china-operator-ip/issues/10)。处于兼容性考虑，当前铁通的预生成数据同中国移动，未来将择机移除铁通。*

*P.S. 鹏博士集团（包括：鹏博士数据、北京电信通、长城宽带、宽带通）的IP地址并非全都由独立的自治域做宣告，目前大部分地址仍由电信、联通、科技网代为宣告。故[列表](https://github.com/gaoyifan/china-operator-ip/blob/ip-lists/drpeng.txt)中的地址仅为鹏博士拥有的部分IP地址，且这些IP同时具有电信、联通两个上级出口。详见[issue #2](https://github.com/gaoyifan/china-operator-ip/issues/2).*

*P.S. 如果需要国内所有地址的集合，请参考 [chnroutes2](https://github.com/misakaio/chnroutes2) 项目*

## 如何获取数据

### 使用预生成结果

IP列表（CIDR格式）保存在仓库的[ip-lists分支](https://gaoyifan.github.io/china-operator-ip/index.html)中，GitHub Actions每日自动更新。

```sh
git clone -b ip-lists https://github.com/gaoyifan/china-operator-ip.git
```

P.S. [stat文件](https://github.com/gaoyifan/china-operator-ip/blob/ip-lists/stat)记录了各运营商的IP数量的统计信息。

### 从BGP数据生成

#### 安装依赖

* [bgptools](https://github.com/gaoyifan/bgptools) (`cargo install bgptools `)
* [bgpdump](https://bitbucket.org/ripencc/bgpdump-hg/wiki/Home) (`apt install bgpdump`)
* [cidr-merger](https://github.com/zhanhb/cidr-merger) (`go get github.com/zhanhb/cidr-merger`)

#### 生成IP列表

```shell
./generate.sh
```

#### 统计IP数量

```shell
./stat.sh
```
## 致谢

* 感谢[boj](https://ring0.me)师兄提出的[设计建议](https://github.com/ustclug/discussions/issues/79#issuecomment-267958775)
* 感谢[University of Oregon Route Views Archive Project](http://archive.routeviews.org)项目提供BGP数据源
* 感谢[Travis CI](https://travis-ci.org)提供优秀的持续集成平台
* 感谢[GitHub](https://github.com/features/actions)提供计算资源
* 感谢[cidr-merger](https://github.com/zhanhb/cidr-merger)项目提供高效的IP地址合并工具
* 感谢[bgpdump](https://bitbucket.org/ripencc/bgpdump/wiki/Home)项目提供rib数据的读取工具

## 协议

[![FOSSA Status](https://app.fossa.io/api/projects/git%2Bgithub.com%2Fgaoyifan%2Fchina-operator-ip.svg?type=large)](https://app.fossa.io/projects/git%2Bgithub.com%2Fgaoyifan%2Fchina-operator-ip?ref=badge_large)
