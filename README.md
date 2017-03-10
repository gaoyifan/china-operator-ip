#中国运营商IP地址列表

依据中国网络运营商分类的IP地址列表

(*本项目仍在开发阶段，数据已经可用，代码和文档有待完善*)

## 特点

* 由BGP/ASN数据分析得到
* 每日更新
* 代码开源，数据免费

## 收录的运营商

* 中国电信(chinanet)
* 中国移动(cmcc)
* 中国联通(unicom)
* 中国铁通(tietong)
* 教育网(cernet)
* 科技网(cstnet)
* 鹏博士(drpeng) <试验阶段>

*P.S. 鹏博士集团（包括：鹏博士数据、北京电信通、长城宽带、宽带通）的IP地址并非全都由独立的自治域做广播，目前大部分地址仍由电信、联通、CNNIC代为广播。故列表(`drpeng.txt`)中的地址仅为鹏博士拥有的部分IP地址，且这些IP同时具有电信、联通两个上级出口。详见[issue #2](https://github.com/gaoyifan/china-operator-ip/issues/2).*

## 如何获取数据

### 使用预生成结果

IP列表（CIDR格式）保存在仓库的[ip-lists分支](https://github.com/gaoyifan/china-operator-ip/tree/ip-lists)中，[Travis CI](https://travis-ci.org)每日自动更新。其中`stat`存储了各运营商的IP数量的统计信息。

### 从BGP数据生成

#### 安装依赖

* [bgptools](https://github.com/gaoyifan/bgptools) (`cargo install bgptools `)
* [docker](https://www.docker.com) (`curl -sSL https://get.docker.com | sh`)

#### 生成IP列表

```shell
./generate.sh
```

#### 统计IP数量

```shell
./stat.sh
```
## 常见使用场景

- DNS分域解析
- 多出口路由器路由表