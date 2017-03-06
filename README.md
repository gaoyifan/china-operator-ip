#中国运营商IP地址列表

依据中国网络运营商分类的IP地址列表

*本项目仍在开发阶段，数据已经可用，代码和文档有待完善*

## 特点

* 每日更新
* 由BGP/ASN数据分析得到
* 代码开源，数据免费

## 常见使用场景

- DNS分域解析
- 多出口路由器路由表

## 收录的运营商

* 中国电信(chinanet)
* 中国移动(cmcc)
* 中国联通(unicom)
* 中国铁通(tietong)
* 教育网(cernet)
* 科技网(cstnet)

## 如何获取数据

### 直接下载

IP列表（CIDR格式）保存在仓库的[result目录](https://github.com/gaoyifan/china-operator-ip/tree/master/result)中。其中`result/stat`存储了各运营商的IP数量的统计信息。

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