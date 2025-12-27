set unstable
set script-interpreter := ['bash']
bgptools_version := "0.3.0"

default: prepare all stat

# Install or update bgp tooling dependencies
[script]
dependency:
  set -euo pipefail

  if ! bgptools --version 2>/dev/null | grep -F "{{bgptools_version}}" >/dev/null; then
    cargo install --force --version "{{bgptools_version}}" bgptools
  fi

  if ! bgpkit-broker --version >/dev/null 2>&1; then
    cargo binstall --secure --no-confirm bgpkit-broker@0.7.0
  fi

  bgptools --version
  bgpkit-broker --version

# Download and normalize latest autnums list
[script]
prepare_autnums:
  set -euo pipefail

  aria2c -s 4 -x 4 -q -o autnums.html --allow-overwrite=true https://bgp.potaroo.net/cidr/autnums.html
  awk -F'[<>]' '{print $3,$5}' autnums.html | grep '^AS' > asnames.txt
  rm -f autnums.html
  echo "INFO> asnames.txt updated ($(wc -l < asnames.txt) entries)" >&2

# Download the latest RIB snapshot for a collector
[script]
prepare_rib collector:
  set -euo pipefail

  url="$(bgpkit-broker latest -c "{{collector}}" --json \
    | jq -r '.[] | select(.data_type | contains("rib")) | .url' \
    | head -n 1)"

  if [[ -z "${url}" ]]; then
    echo "Unable to determine {{collector}} RIB download url" >&2
    exit 1
  fi

  if [[ "${url}" =~ (\.gz|\.bz2)$ ]]; then
    suffix="${BASH_REMATCH[1]}"
  else
    echo "Unsupported archive format for {{collector}}: ${url}" >&2
    exit 1
  fi

  outfile="rib-{{collector}}${suffix}"

  rm -f "${outfile}"
  aria2c -s 4 -x 4 -q -o "${outfile}" "${url}"
  stat "${outfile}"
  echo "INFO> ${outfile} ready for bgptools" >&2

# Download the latest RIB snapshots (rrc21, rrc12, route-views6)
[parallel]
prepare_ribs: (prepare_rib "rrc00") (prepare_rib "rrc21") (prepare_rib "rrc12") (prepare_rib "route-views6")

# Prepare data for generation
[parallel]
prepare: prepare_autnums prepare_ribs

# Print ASN list for OPERATOR based on operators.yaml
get_asn operator:
  #!/usr/bin/env ruby
  require "yaml"

  cfg_path = "operators.yaml"
  asnames = "asnames.txt"
  abort("Missing config: #{cfg_path}") unless File.file?(cfg_path)
  abort("Missing asnames.txt. Run 'just prepare_autnums' first.") unless File.file?(asnames) && File.size?(asnames)

  op = YAML.load_file(cfg_path).fetch("operators").fetch("{{operator}}")
  pat = op["pattern"].to_s
  ex = op.fetch("exclude", "^$")
  if (ex_asn = op.fetch("exclude_asn", [])).any?
    list = "^AS(#{ex_asn.join("|")})\\b"
    ex = (ex && ex != "^$") ? "(?:#{ex})|(?:#{list})" : list
  end
  pat = pat.empty? ? nil : Regexp.new(pat, Regexp::IGNORECASE)
  ex  = (ex && ex != "^$") ? Regexp.new(ex, Regexp::IGNORECASE) : nil
  asn = /^AS(\d+)/

  File.foreach(asnames) do |l|
    l.chomp!
    next unless l.end_with?(op.fetch("country"))
    next if pat && !pat.match?(l)
    next if ex && ex.match?(l)
    puts asn.match(l)[1] if asn.match?(l)
  end

# Generate IP lists for a single operator
gen operator:
  #!/usr/bin/env ruby
  require "fileutils"

  operator = "{{operator}}"
  FileUtils.mkdir_p("result")

  out = "result/#{operator}46.txt"
  v4 = "result/#{operator}.txt"
  v6 = "result/#{operator}6.txt"

  ribs = Dir.glob("rib-*.{gz,bz2}").sort
  abort("No rib-*.gz or rib-*.bz2 files found. Run 'just prepare_ribs' first.") if ribs.empty?

  bgptools = ["bgptools", "--ignore-private-asn", "--cache"] + ribs.flat_map { |r| ["--mrt-file", r] }

  warn "INFO> #{operator} start"
  asns = IO.popen(["just", "get_asn", operator], &:read)
  abort("Failed to get ASN list for #{operator}") unless $?.success?
  asn_list = asns.split
  ok = system(*bgptools, *asn_list, out: out)
  abort("Failed to run bgptools for #{operator}") unless ok

  lines = File.read(out).lines
  v6_lines, v4_lines = lines.partition { |line| line.include?(":") }
  File.write(v4, v4_lines.join)
  File.write(v6, v6_lines.join)
  warn "INFO> #{operator} done (v4=#{v4_lines.length} v6=#{v6_lines.length})"

# Generate IP lists for all operators sequentially
all:
  #!/usr/bin/env ruby
  require "yaml"

  ops = YAML.load_file("operators.yaml").fetch("operators").keys.sort
  ops.each do |op|
    status = system("just", "gen", op)
    exit($?.exitstatus || 1) unless status
  end

[script]
guard:
  set -euo pipefail

  if [[ $(wc -l < result/china.txt) -lt 3000 ]]; then
    echo "china.txt too small" >&2
    exit 1
  fi

  if [[ $(wc -l < result/china6.txt) -lt 1000 ]]; then
    echo "china6.txt too small" >&2
    exit 2
  fi

  echo "INFO> guard checks passed" >&2

# Summarize total IPv4/IPv6 address space per operator
stat:
  #!/usr/bin/env ruby
  dir = "result"
  files = Dir.exist?(dir) ? Dir.glob("#{dir}/*.txt").sort : []
  files.reject! { |p| p.end_with?("46.txt") }
  abort("result/*.txt files missing") if files.empty?

  mask = %r{/(\d+)}
  report = files.map do |p|
    base = p.end_with?("6.txt") ? 48 : 32
    total = File.foreach(p).sum do |line|
      m = mask.match(line)
      m && m[1].to_i <= base ? (1 << (base - m[1].to_i)) : 0
    end
    "#{File.basename(p, ".txt")}\n#{total}"
  end.join("\n\n") + "\n"

  print report
  File.write("#{dir}/stat", report)

# Publish generated results into the ip-lists branch
[script]
upload: guard
  set -euo pipefail
  rm -f ip-lists/*.txt
  mv result/* ip-lists
  cd ip-lists
  tree -H . -P "*.txt|stat" -T "China Operator IP - prebuild results" > index.html
  git config user.name "GitHub Actions"
  git config user.email noreply@github.com
  git add .
  git commit -m "update $(date +%Y-%m-%d)"
  git push -q

# Refresh CDN cache for all files in ip-lists directory
[script]
refresh_jsdelivr repository:
  set -euo pipefail

  if [[ ! -d ip-lists ]]; then
    echo "ip-lists directory not found" >&2
    exit 1
  fi

  cd ip-lists
  for file in *; do
    if [[ -f "${file}" ]]; then
      echo "INFO> purging CDN cache for ${file}" >&2
      curl -i "https://purge.jsdelivr.net/gh/{{repository}}@ip-lists/${file}"
    fi
  done
