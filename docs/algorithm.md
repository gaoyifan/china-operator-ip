## Algorithm Overview

The built-in BGP classifier reads one or more MRT/RIB files and outputs the IPv4/IPv6 prefixes originated by the ASNs provided on the command line.

- Inputs: MRT file paths (`--mrt-file`), target ASNs (positional), `--ignore-private-asn`, `--origin-only`, `--cache`, an optional low-priority CIDR fallback list (`--fallback-prefix-file`), an optional foreign-upstream filter (`--exclude-foreign-upstream-only` + `--asn-country-file`), and an optional trusted-CN-transit filter (`--trusted-cn-transit-file` + `--asn-country-file`, requires `--origin-only`).
- Output: Simplified, sorted list of CIDR prefixes (v4 then v6) for the requested ASNs.

## Processing Steps

1) **Parse MRT files (parallel)**
   Each MRT file is parsed with `BgpkitParser`, keeping only ANNOUNCE records. For every prefix the code collects:
   - Origin ASNs (skipping private ASNs when requested).
   - AS path (truncated to last 4 hops).
   - Split points: prefix network address and the next address after the broadcast. These points mark boundaries for later interval construction.
   Results are stored separately for v4 and v6:
   - `announced_*`: every non-default announced prefix, collected before origin, private-ASN, country, or transit filtering. IPv4/IPv6 default routes are excluded because they do not establish a globally routed address assignment.
   - `prefix_map_*`: longest-prefix-match map of prefix → set of origin ASNs.
   - `as_paths_*`: prefix → origin ASN → list of truncated AS paths.
   - `direct_upstreams`: origin ASN → set of observed direct upstream ASNs.
   - `split_points_*`: ordered set of addresses that delimit intervals.

2) **Merge per-file data**
   Parsed structures are merged across files. Split points are deduped and sorted (via `BTreeSet`).

3) **Optional trusted-CN-transit filtering**
   With `--origin-only`, inspect every observed AS path from the origin side. An origin ASN is retained, along with all of its prefixes, when at least one path has a contiguous CN suffix containing an ASN from `--trusted-cn-transit-file`. Inspection stops at the first non-CN or unknown ASN, so a trusted CN network beyond foreign transit does not make the origin domestic. The CLI rejects this filter without `--origin-only` because shared-upstream attribution is a different mode.

4) **Add shared upstream ASNs**
   Unless `--origin-only` is set, the algorithm computes the longest common suffix of the collected AS paths (capped to 4 ASNs). These shared upstream ASNs are added to the prefix map so they are treated like origin ASNs for interval attribution.

5) **Build ASN → IP ranges**
   Consecutive split points define half-open intervals `[start, end)`. For each interval, a /32 (v4) or /128 (v6) lookup finds the longest covering prefix and its ASNs. Each ASN receives the interval, converted to a minimal set of CIDRs by the generic address-family implementation. The per-AS ranges are stored as `IpRange` structures to allow merging.

6) **Optional foreign-upstream filtering**
   When `--exclude-foreign-upstream-only <COUNTRY>` is enabled, the classifier loads ASN → country data from `--asn-country-file` and removes any requested ASN whose observed direct upstream ASNs are all known and all outside `<COUNTRY>`. A hidden debug flag can print this matched ASN list directly.

7) **Apply registration fallbacks**
   Each CIDR from `--fallback-prefix-file` contributes only its difference from the complete observed announcement set. This gives BGP higher priority than fallback data: `result = classified ∪ (fallback − announced)`. Any announcement, regardless of ASN or classification, blocks fallback coverage for that space.

8) **Finalize result**
   For the remaining requested ASNs, the collected ranges and unannounced fallbacks are merged and simplified, then emitted in sorted order (v4 then v6).

## Caching

When `--cache` is enabled, the computed ASN→range maps, complete observed announcement sets, and origin→direct-upstream map are serialized to a bincode file keyed by input file list, `ignore_private_asn`, `origin_only`, and the trusted-transit policy fingerprint. The fallback file is read after cache loading and therefore does not affect the key. Subsequent runs reuse the cache when the key matches.

## Modules

- `src/main.rs`: Parses CLI options, selects cache use and prints the final result.
- `src/classifier.rs`: Parses MRT files and exposes the completed classification through `Classification`.
- `src/ip.rs`: Owns dual-stack state and the generic address-family implementation for interval slicing, longest-prefix lookup and fallback subtraction.
- `src/asn.rs`: Defines the `Asn` domain type, AS-path operations, domestic policy and country/upstream classification.
- `src/cache.rs`: Owns cache keys, format version 5 and bincode persistence.
