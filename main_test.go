package main

import (
	"testing"
	"bufio"
	"strings"
	"net"
)

type expected struct {
	ip string
	operator string
}

var (
	M map[byte]IPList
	F *Finder
	Tests []expected
)

func init() {
	M = map[byte]IPList{
		0: IPList{
			name: "net0",
			content: bufio.NewScanner(strings.NewReader("1.1.0.0/16\n1.2.0.0/25")),
		},
		1: IPList{
			name: "net1",
			content: bufio.NewScanner(strings.NewReader("1.1.1.0/24")),
		},
		2: IPList{
			name: "net2",
			content: bufio.NewScanner(strings.NewReader("1.1.2.0/24")),
		},
	}

	F = NewFinder(M)
	F.Build()

	Tests = []expected{
		{"1.1.1.1", "net1"},
		{"1.1.2.1", "net2"},
		{"1.1.3.1", "net0"},
		{"1.2.0.64", "net0"},
		{"1.2.0.128", ""},
	}
}

func TestFinder(t *testing.T) {
	for i, tt := range Tests {
		actual := F.Search(net.ParseIP(tt.ip))
		if actual != tt.operator {
			t.Errorf("%d: expected %s, got %s", i, tt.operator, actual)
		}
	}
}

func BenchmarkFinder(b *testing.B) {
	ip := net.ParseIP("1.1.3.4")
	for i := 0; i < b.N; i++ {
		F.Search(ip)
	}
}
