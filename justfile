set unstable
bgptools_version := "0.3.0"

default: prepare all stat

# Install or update bgp tooling dependencies
dependency:
  #!/usr/bin/env bash
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
prepare_autnums:
  #!/usr/bin/env bash
  set -euo pipefail

  aria2c -s 4 -x 4 -q -o autnums.html --allow-overwrite=true https://bgp.potaroo.net/cidr/autnums.html
  awk -F'[<>]' '{print $3,$5}' autnums.html | grep '^AS' > asnames.txt
  rm -f autnums.html
  echo "INFO> asnames.txt updated ($(wc -l < asnames.txt) entries)" >&2

# Download the latest RIB snapshot for a collector
prepare_rib collector:
  #!/usr/bin/env bash
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

  cfg, asnames = "operators.yaml", "asnames.txt"
  abort("Missing config: #{cfg}") unless File.file?(cfg)
  abort("Missing asnames.txt. Run 'just prepare_autnums' first.") unless File.file?(asnames) && File.size?(asnames)

  op = YAML.load_file(cfg).fetch("operators").fetch("{{operator}}")
  country = op.fetch("country")
  exclude_asn = op.fetch("exclude_asn", []).map(&:to_s)
  pattern_re = Regexp.new(op["pattern"].to_s, Regexp::IGNORECASE)
  exclude_re = Regexp.new(op.fetch("exclude", "^$"), Regexp::IGNORECASE)

  File.foreach(asnames) do |line|
    line.chomp!
    match = line.match(/^AS(\d+)\b.*,\s*([A-Z]{2})$/)
    asn, line_country = match&.captures
    next unless line_country == country
    next if exclude_asn.include?(asn)
    next unless pattern_re.match?(line)
    next if exclude_re.match?(line)
    puts asn
  end

# Generate IP lists for a single operator
gen operator:
  #!/usr/bin/env ruby
  require "fileutils"

  operator = "{{operator}}"
  FileUtils.mkdir_p("result")
  out, v4, v6 = %W[result/#{operator}46.txt result/#{operator}.txt result/#{operator}6.txt]

  ribs = Dir["rib-*.{gz,bz2}"].sort
  abort("No rib-*.gz or rib-*.bz2 files found. Run 'just prepare_ribs' first.") if ribs.empty?
  bgptools = ["bgptools", "--ignore-private-asn", "--cache"] + ribs.flat_map { |r| ["--mrt-file", r] }

  warn "INFO> #{operator} start"
  asns = IO.popen(["just", "get_asn", operator], &:read)
  abort("Failed to get ASN list for #{operator}") unless $?.success?
  abort("Failed to run bgptools for #{operator}") unless system(*bgptools, *asns.split, out: out)

  v6_lines, v4_lines = File.readlines(out).partition { |line| line.include?(":") }
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

guard:
  #!/usr/bin/env ruby
  {"china.txt" => 3000, "china6.txt" => 1000}.each do |f, min|
    next if File.foreach("result/#{f}").count >= min
    warn "#{f} too small"
    exit 1
  end
  warn "INFO> guard checks passed"

# Summarize total IPv4/IPv6 address space per operator
stat:
  #!/usr/bin/env ruby
  dir = "result"
  files = Dir.exist?(dir) ? Dir.glob("#{dir}/*.txt").sort : []
  files.reject! { |p| p.end_with?("46.txt") }
  abort("result/*.txt files missing") if files.empty?

  report = files.map do |p|
    base = p.end_with?("6.txt") ? 48 : 32
    total = File.foreach(p).sum do |line|
      match = %r{/(\d+)}.match(line)
      next 0 unless match
      prefix_len = match[1].to_i
      prefix_len <= base ? (1 << (base - prefix_len)) : 0
    end
    "#{File.basename(p, ".txt")}\n#{total}"
  end.join("\n\n") + "\n"

  print report
  File.write("#{dir}/stat", report)

# Publish generated results into the ip-lists branch
upload: guard
  #!/usr/bin/env bash
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
refresh_jsdelivr repository:
  #!/usr/bin/env ruby
  require "net/http"

  dir = "ip-lists"
  abort("#{dir} directory not found") unless Dir.exist?(dir)

  Dir.children(dir).sort.each do |file|
    warn "INFO> purging CDN cache for #{file}"
    puts Net::HTTP.get_response(URI("https://purge.jsdelivr.net/gh/{{repository}}@#{dir}/#{file}")).inspect
  end
