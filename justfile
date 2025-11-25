set unstable
set script-interpreter := ['bash']
bgptools_version := "0.2.2"

default: prepare all stat

[doc('Install or update bgp tooling dependencies')]
[script]
dependency:
  set -euo pipefail

  if ! bgptools --version 2>/dev/null | grep -F "{{bgptools_version}}" >/dev/null; then
    cargo install --version "{{bgptools_version}}" bgptools
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

[doc('Download the latest IPv4 RIB snapshot')]
[script]
prepare_rib_v4:
  set -euo pipefail

  url="$(bgpkit-broker latest -c rrc00 --json \
    | jq -r '.[] | select(.data_type | contains("rib")) | .url' \
    | head -n 1)"

  if [[ -z "${url}" ]]; then
    echo "Unable to determine IPv4 RIB download url" >&2
    exit 1
  fi

  rm -f rib.gz
  axel -q -o rib.gz "${url}"
  stat rib.gz
  echo "INFO> rib.gz ready for bgptools" >&2

[doc('Download the latest IPv6 RIB snapshot')]
[script]
prepare_rib_v6:
  set -euo pipefail

  url="$(bgpkit-broker latest -c route-views6 --json \
    | jq -r '.[] | select(.data_type | contains("rib")) | .url' \
    | head -n 1)"

  if [[ -z "${url}" ]]; then
    echo "Unable to determine IPv6 RIB download url" >&2
    exit 1
  fi

  rm -f rib6.bz2
  axel -q -o rib6.bz2 "${url}"
  stat rib6.bz2
  echo "INFO> rib6.bz2 ready for bgptools" >&2

[doc('Prepare data for generation')]
[parallel]
prepare: prepare_autnums prepare_rib_v4 prepare_rib_v6

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
[parallel]
gen operator: (gen4 operator) (gen6 operator)

[script]
gen4 operator:
  set -euo pipefail

  mkdir -p result

  echo "INFO> generating IPv4 prefixes for {{operator}}" >&2
  just get_asn "{{operator}}" \
    | tee >(awk 'END { if (NR == 0) exit 1 }') \
    | xargs bgptools --mrt-file rib.gz \
    | grep -Fv ':' \
    > "result/{{operator}}.txt"
  echo "INFO> {{operator}}.txt generated ($(wc -l < result/{{operator}}.txt) entries)" >&2

[script]
gen6 operator:
  set -euo pipefail

  mkdir -p result

  echo "INFO> generating IPv6 prefixes for {{operator}}" >&2
  just get_asn "{{operator}}" \
    | tee >(awk 'END { if (NR == 0) exit 1 }') \
    | xargs bgptools --mrt-file rib6.bz2 \
    | grep -v '^::/0$' \
    | grep -F ':' \
    > "result/{{operator}}6.txt" || true  # ignore empty output, since drpeng has no IPv6 prefixes
  echo "INFO> {{operator}}6.txt generated ($(wc -l < result/{{operator}}6.txt) entries)" >&2

[doc('Generate IP lists for all operators sequentially')]
[parallel]
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
[script]
stat:
  set -euo pipefail
  cd result

  for file in *.txt; do
    name="${file%.*}"
    echo "${name}"
    if [[ "${file}" == *6.txt ]]; then
      base=48
    else
      base=32
    fi

    sum=0
    while IFS=/ read -r _ mask; do
      if [[ -z "${mask}" ]]; then
        continue
      fi
      if (( mask <= base )); then
        ((s=base-mask))
        ((sum+=1<<s))
      fi
    done < "${file}"
    echo "${sum}"
    echo
  done | tee stat

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

