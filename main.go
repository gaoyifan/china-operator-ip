package main

import (
	"bufio"
	"fmt"
	"log"
	"net"
	"net/http"
	"os"

	"github.com/gorilla/mux"
)

const (
	CHINANET byte = iota
	CMCC
	UNICOM
	TIETONG
	CERNET
	CSTNET
	DRPENG
)

type TrieNode struct {
	End   bool
	Label byte
	CIDR  string
	Succ  [2]*TrieNode
}

type IPList struct {
	name    string
	content *bufio.Scanner
}

type Finder struct {
	root      *TrieNode
	ipListMap map[byte]IPList
}

func NewFinder(m map[byte]IPList) *Finder {
	return &Finder{&TrieNode{}, m}
}

func IP2uint(ip net.IP) uint32 {
	return (uint32(ip[0]) << 24) | (uint32(ip[1]) << 16) | (uint32(ip[2]) << 8) | uint32(ip[3])
}

func (f *Finder) Build() {
	for label, ipList := range f.ipListMap {
		scanner := ipList.content
		for scanner.Scan() {
			root := f.root
			seg := scanner.Text()
			_, n, err := net.ParseCIDR(seg)
			if err != nil {
				log.Fatal(err)
			}
			ipInt := IP2uint(n.IP)
			var b uint32 = 1 << 31
			ones, _ := n.Mask.Size()
			for i := 0; i < ones; i++ {
				k := 0
				if b&ipInt != 0 {
					k = 1
				}
				if root.Succ[k] == nil {
					root.Succ[k] = new(TrieNode)
				}
				root = root.Succ[k]
				b = b >> 1
			}
			root.End = true
			root.Label = label
			root.CIDR = n.String()
		}
	}
}

func (f *Finder) Search(ip net.IP) string {
	ip = ip.To4()
	if ip == nil {
		return ""
	}
	ipInt := IP2uint(ip)
	root := f.root
	var prev, next *TrieNode
	for b := uint32(1 << 31); b > 0; b = b >> 1 {
		if ipInt&b == 0 {
			next = root.Succ[0]
		} else {
			next = root.Succ[1]
		}
		if root.End {
			prev = root
			if next == nil {
				return f.ipListMap[root.Label].name
			}
		} else if next == nil {
			break
		}
		root = next
	}
	if prev != nil {
		return f.ipListMap[prev.Label].name
	}
	return ""
}

func main() {
	type Info struct {
		path  string
		name  string
		label byte
		file  *os.File
	}
	infos := []Info{
		{"./chinanet.txt", "中国电信", CHINANET, nil},
		{"./cmcc.txt", "中国移动", CMCC, nil},
		{"./unicom.txt", "中国联通", UNICOM, nil},
		{"./tietong.txt", "中国铁通", TIETONG, nil},
		{"./cernet.txt", "教育网", CERNET, nil},
		{"./cstnet.txt", "科技网", CSTNET, nil},
		{"./drpeng.txt", "鹏博士", DRPENG, nil},
	}
	m := make(map[byte]IPList)
	for i := 0; i < len(infos); i++ {
		x := infos[i]
		f, err := os.Open(x.path)
		if err != nil {
			log.Fatal(err)
		}
		x.file = f
		m[x.label] = IPList{
			name:    x.name,
			content: bufio.NewScanner(f),
		}
	}
	F := NewFinder(m)
	F.Build()
	for i := 0; i < len(infos); i++ {
		infos[i].file.Close()
	}
	r := mux.NewRouter()
	r.HandleFunc("/{ip}", func(w http.ResponseWriter, r *http.Request) {
		vars := mux.Vars(r)
		rawip := vars["ip"]
		ip := net.ParseIP(rawip).To4()
		if ip == nil {
			w.WriteHeader(http.StatusBadRequest)
			w.Write([]byte("Invalid IP\n"))
			return
		}
		o := F.Search(ip)
		if o != "" {
			w.Write([]byte(fmt.Sprintf("%s 属于 %s\n", rawip, o)))
		} else {
			w.Write([]byte(fmt.Sprintf("找不到该 IP: %s\n", rawip)))
		}
	})
	addr := "127.0.0.1:10080"
	log.Printf("Listening on %s", addr)
	log.Fatal(http.ListenAndServe(addr, r))
}
