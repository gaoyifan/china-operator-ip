set unstable
set script-interpreter := ['bash']
bgptools_version := "0.2.4"

default: prepare all stat

[doc('Install or update bgp tooling dependencies')]
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

[doc('Download and normalize latest autnums list')]
[script]
prepare_autnums:
  set -euo pipefail

  axel -q -o autnums.html https://bgp.potaroo.net/cidr/autnums.html
  awk -F'[<>]' '{print $3,$5}' autnums.html | grep '^AS' > asnames.txt
  rm -f autnums.html
  echo "INFO> asnames.txt updated ($(wc -l < asnames.txt) entries)" >&2

[doc('Download the latest RIB snapshot for a collector')]
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
  axel -q -o "${outfile}" "${url}"
  stat "${outfile}"
  echo "INFO> ${outfile} ready for bgptools" >&2

[doc('Download the latest RIB snapshots (rrc21, rrc12, route-views6)')]
[parallel]
prepare_ribs: (prepare_rib "rrc00") (prepare_rib "rrc21") (prepare_rib "rrc12") (prepare_rib "route-views6")

[doc('Prepare data for generation')]
[parallel]
prepare: prepare_autnums prepare_ribs

[doc('Print ASN list for OPERATOR based on operator/*.conf')]
[script]
get_asn operator:
  set -euo pipefail

  config="operator/{{operator}}.conf"
  if [[ ! -f "${config}" ]]; then
    echo "Unknown operator: {{operator}}" >&2
    exit 1
  fi

  if [[ ! -s asnames.txt ]]; then
    echo "Missing asnames.txt. Run 'just prepare_autnums' first." >&2
    exit 1
  fi

  # shellcheck disable=SC1090
  source "${config}"
  : "${COUNTRY:?COUNTRY must be set in ${config}}"
  EXCLUDE="${EXCLUDE:-^$}"
  PATTERN="${PATTERN:-}"

  grep -P "${COUNTRY}\$" asnames.txt \
    | grep -Pi "${PATTERN}" \
    | grep -vPi "${EXCLUDE}" \
    | awk '{gsub(/AS/, ""); print $1 }'

[doc('Generate IP lists for a single operator')]
gen operator: (gen4 operator) (gen6 operator)

[script]
gen4 operator:
  set -euo pipefail

  mkdir -p result

  RIB_FILES=()
  for rib in rib-*.gz rib-*.bz2; do
    [[ -f "${rib}" ]] || continue
    RIB_FILES+=("--mrt-file" "${rib}")
  done

  if [[ ${#RIB_FILES[@]} -eq 0 ]]; then
    echo "No rib-*.gz or rib-*.bz2 files found. Run 'just prepare_ribs' first." >&2
    exit 1
  fi

  echo "INFO> generating IPv4 prefixes for {{operator}}" >&2
  just get_asn "{{operator}}" \
    | tee >(awk 'END { if (NR == 0) exit 1 }') \
    | xargs bgptools --ignore-private-asn --cache "${RIB_FILES[@]}" \
    | grep -Fv ':' \
    > "result/{{operator}}.txt"
  echo "INFO> {{operator}}.txt generated ($(wc -l < result/{{operator}}.txt) entries)" >&2

[script]
gen6 operator:
  set -euo pipefail

  mkdir -p result

  RIB_FILES=()
  for rib in rib-*.gz rib-*.bz2; do
    [[ -f "${rib}" ]] || continue
    RIB_FILES+=("--mrt-file" "${rib}")
  done

  if [[ ${#RIB_FILES[@]} -eq 0 ]]; then
    echo "No rib-*.gz or rib-*.bz2 files found. Run 'just prepare_ribs' first." >&2
    exit 1
  fi

  echo "INFO> generating IPv6 prefixes for {{operator}}" >&2
  just get_asn "{{operator}}" \
    | tee >(awk 'END { if (NR == 0) exit 1 }') \
    | xargs bgptools --ignore-private-asn --cache "${RIB_FILES[@]}" \
    | grep -v '^::/0$' \
    | grep -F ':' \
    > "result/{{operator}}6.txt" || true  # ignore empty output, since drpeng has no IPv6 prefixes
  echo "INFO> {{operator}}6.txt generated ($(wc -l < result/{{operator}}6.txt) entries)" >&2

[doc('Generate IP lists for all operators sequentially')]
all: (gen "china") (gen "cernet") (gen "chinanet") (gen "cmcc") (gen "unicom") (gen "cstnet") (gen "drpeng") (gen "googlecn")

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

[doc('Summarize total IPv4/IPv6 address space per operator')]
stat:
  #!/usr/bin/env python3
  import re, sys
  from pathlib import Path

  result_dir = Path("result")
  files = sorted(result_dir.glob("*.txt")) if result_dir.is_dir() else []
  if not files:
    sys.exit("result/*.txt files missing")

  mask = re.compile(r"/(\d+)")

  def seats(path):
    base = 48 if path.name.endswith("6.txt") else 32
    with path.open() as fh:
      masks = (int(m.group(1)) for line in fh if (m := mask.search(line)))
      total = sum(1 << (base - m) for m in masks if m <= base)
    return path.stem, total

  report = "\n\n".join(f"{name}\n{total}" for name, total in map(seats, files)) + "\n"
  sys.stdout.write(report)
  (result_dir / "stat").write_text(report)

[doc('Publish generated results into the ip-lists branch')]
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

[doc('Refresh CDN cache for all files in ip-lists directory')]
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

