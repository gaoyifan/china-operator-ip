use bgpkit_parser::{BgpkitParser, models::ElemType};
use clap::{ArgAction, Parser};
use ipnet::{IpNet, Ipv4Net, Ipv4Subnets, Ipv6Net, Ipv6Subnets};
use iprange::{IpNet as IpRangeNet, IpRange, ToNetwork};
use prefix_trie::PrefixMap;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::collections::{BTreeSet, HashMap, HashSet, hash_map::DefaultHasher};
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, BufWriter};
use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::{Path, PathBuf};
use vec_collections::VecSet;

type AsnRangesV4 = HashMap<u32, IpRange<Ipv4Net>>;
type AsnRangesV6 = HashMap<u32, IpRange<Ipv6Net>>;
type DirectUpstreams = HashMap<u32, BTreeSet<u32>>;
const CACHE_FORMAT_VERSION: u8 = 4;

struct DomesticPolicy {
    trusted_transit_asns: HashSet<u32>,
    asn_countries: HashMap<u32, String>,
}

struct ParsedMrtData {
    prefix_map_v4: PrefixMap<Ipv4Net, VecSet<[u32; 4]>>,
    prefix_map_v6: PrefixMap<Ipv6Net, VecSet<[u32; 4]>>,
    announced_v4: IpRange<Ipv4Net>,
    announced_v6: IpRange<Ipv6Net>,
    as_paths_v4: HashMap<Ipv4Net, HashMap<u32, Vec<SmallVec<[u32; 4]>>>>,
    as_paths_v6: HashMap<Ipv6Net, HashMap<u32, Vec<SmallVec<[u32; 4]>>>>,
    domestic_origins: HashSet<u32>,
    direct_upstreams: DirectUpstreams,
    split_points_v4: BTreeSet<Ipv4Addr>,
    split_points_v6: BTreeSet<Ipv6Addr>,
}

fn is_private_asn(asn: u32) -> bool {
    (64512..=65534).contains(&asn) || (4_200_000_000..=4_294_967_294).contains(&asn)
}

#[derive(Parser, Debug)]
#[command(name = "china-operator-ip", version)]
struct Opts {
    #[arg(short, long = "mrt-file", value_name = "MRT", action = ArgAction::Append)]
    mrt_files: Vec<PathBuf>,

    #[arg(value_name = "ASN", value_parser = clap::value_parser!(u32), num_args = 1..)]
    asns: Vec<u32>,

    #[arg(long, default_value_t = false)]
    ignore_private_asn: bool,

    #[arg(long, default_value_t = false)]
    origin_only: bool,

    #[arg(long, default_value_t = false)]
    cache: bool,

    #[arg(long, value_name = "FILE")]
    fallback_prefix_file: Option<PathBuf>,

    #[arg(long, value_name = "COUNTRY")]
    exclude_foreign_upstream_only: Option<String>,

    #[arg(long, value_name = "FILE")]
    asn_country_file: Option<PathBuf>,

    #[arg(
        long,
        hide = true,
        value_name = "FILE",
        requires_all = ["origin_only", "asn_country_file"]
    )]
    trusted_cn_transit_file: Option<PathBuf>,

    #[arg(long, hide = true, default_value_t = false)]
    debug_print_foreign_upstream_only_asns: bool,

    #[arg(long, hide = true, default_value_t = false)]
    debug_print_seen_origin_asns: bool,
}

fn main() {
    let Opts {
        mrt_files,
        asns,
        ignore_private_asn,
        origin_only,
        cache,
        fallback_prefix_file,
        exclude_foreign_upstream_only,
        asn_country_file,
        trusted_cn_transit_file,
        debug_print_foreign_upstream_only_asns,
        debug_print_seen_origin_asns,
    } = Opts::parse();
    let asn_list: HashSet<u32> = asns.into_iter().collect();
    let domestic_policy = trusted_cn_transit_file.as_deref().map(|trusted_path| {
        let country_path = asn_country_file
            .as_deref()
            .expect("--asn-country-file is required with --trusted-cn-transit-file");
        DomesticPolicy {
            trusted_transit_asns: load_asn_set(trusted_path)
                .unwrap_or_else(|err| panic!("failed to load trusted CN transit ASNs: {err}")),
            asn_countries: load_asn_countries(country_path).unwrap_or_else(|err| {
                panic!(
                    "failed to load ASN country data from {}: {err}",
                    country_path.display()
                )
            }),
        }
    });
    let domestic_policy_fingerprint = domestic_policy.as_ref().map(domestic_policy_fingerprint);

    let AsnData {
        v4: asn_ranges_v4,
        v6: asn_ranges_v6,
        announced_v4,
        announced_v6,
        direct_upstreams,
    } = if cache {
        let cache_path = cache_path(
            &mrt_files,
            ignore_private_asn,
            origin_only,
            domestic_policy_fingerprint,
        );
        load_cache(
            &cache_path,
            ignore_private_asn,
            origin_only,
            domestic_policy_fingerprint,
        )
        .unwrap_or_else(|| {
            let data = build_asn_data(
                &mrt_files,
                ignore_private_asn,
                origin_only,
                domestic_policy.as_ref(),
            );
            let cached = CachedRanges {
                version: CACHE_FORMAT_VERSION,
                ignore_private_asn,
                origin_only,
                domestic_policy_fingerprint,
                data,
            };
            save_cache(&cache_path, cached).data
        })
    } else {
        build_asn_data(
            &mrt_files,
            ignore_private_asn,
            origin_only,
            domestic_policy.as_ref(),
        )
    };

    let foreign_upstream_only_asns = match exclude_foreign_upstream_only.as_deref() {
        Some(country) => {
            let asn_country_file = asn_country_file
                .as_deref()
                .expect("--asn-country-file is required with --exclude-foreign-upstream-only");
            let asn_countries = load_asn_countries(asn_country_file).unwrap_or_else(|err| {
                panic!(
                    "failed to load ASN country data from {}: {err}",
                    asn_country_file.display()
                )
            });
            foreign_upstream_only_asns(&asn_list, &direct_upstreams, &asn_countries, country)
        }
        None => {
            assert!(
                !debug_print_foreign_upstream_only_asns,
                "--debug-print-foreign-upstream-only-asns requires --exclude-foreign-upstream-only"
            );
            Vec::new()
        }
    };

    if debug_print_foreign_upstream_only_asns {
        for asn in foreign_upstream_only_asns {
            println!("{asn}");
        }
        return;
    }

    if debug_print_seen_origin_asns {
        let mut seen_asns: Vec<u32> = asn_list
            .iter()
            .copied()
            .filter(|asn| asn_ranges_v4.contains_key(asn) || asn_ranges_v6.contains_key(asn))
            .collect();
        seen_asns.sort_unstable();
        for asn in seen_asns {
            println!("{asn}");
        }
        return;
    }
    let excluded_asns: HashSet<u32> = foreign_upstream_only_asns.into_iter().collect();

    let mut result_v4: IpRange<Ipv4Net> = IpRange::new();
    let mut result_v6: IpRange<Ipv6Net> = IpRange::new();

    // Step 5: Filter and merge IP ranges for target ASNs
    for asn in &asn_list {
        if excluded_asns.contains(asn) {
            continue;
        }
        if let Some(range) = asn_ranges_v4.get(asn) {
            for net in range.iter() {
                result_v4.add(net);
            }
        }
        if let Some(range) = asn_ranges_v6.get(asn) {
            for net in range.iter() {
                result_v6.add(net);
            }
        }
    }

    let fallback_prefixes = fallback_prefix_file
        .as_deref()
        .map(|path| {
            load_prefixes(path).unwrap_or_else(|err| {
                panic!(
                    "failed to load fallback prefixes from {}: {err}",
                    path.display()
                )
            })
        })
        .unwrap_or_default();
    apply_fallback_prefixes(
        &mut result_v4,
        &mut result_v6,
        &announced_v4,
        &announced_v6,
        &fallback_prefixes,
    );

    result_v4.simplify();
    result_v6.simplify();

    emit_sorted(&result_v4);
    emit_sorted(&result_v6);
}

/// Convert an IP interval [start, end) to a list of CIDR prefixes.
fn interval_to_cidrs_v4(start: Ipv4Addr, end: Ipv4Addr) -> Vec<Ipv4Net> {
    if start >= end {
        return Vec::new();
    }

    let end_inclusive = u32::from(end).saturating_sub(1);
    Ipv4Subnets::new(start, Ipv4Addr::from(end_inclusive), 0).collect()
}

/// Convert an IP interval [start, end) to a list of CIDR prefixes.
fn interval_to_cidrs_v6(start: Ipv6Addr, end: Ipv6Addr) -> Vec<Ipv6Net> {
    if start >= end {
        return Vec::new();
    }

    let end_inclusive = u128::from(end).saturating_sub(1);
    Ipv6Subnets::new(start, Ipv6Addr::from(end_inclusive), 0).collect()
}

/// Compute the longest common suffix of a collection of AS paths, capped to the last 4 elements.
fn longest_common_suffix(paths: &[SmallVec<[u32; 4]>]) -> SmallVec<[u32; 4]> {
    if paths.is_empty() {
        return SmallVec::new();
    }

    let min_len = paths.iter().map(|p| p.len()).min().unwrap_or(0).min(4);
    let mut suffix: SmallVec<[u32; 4]> = SmallVec::new();

    for i in 0..min_len {
        let idx = paths[0].len().saturating_sub(1 + i);
        let candidate = paths[0][idx];
        if paths
            .iter()
            .all(|path| path[path.len().saturating_sub(1 + i)] == candidate)
        {
            suffix.push(candidate);
        } else {
            break;
        }
    }

    suffix.reverse();
    suffix
}

fn normalize_as_path(path: &[u32]) -> SmallVec<[u32; 4]> {
    let mut deduped: SmallVec<[u32; 4]> = SmallVec::new();
    for &asn in path {
        if deduped.last().copied() != Some(asn) {
            deduped.push(asn);
        }
    }
    if deduped.len() > 4 {
        let len = deduped.len();
        SmallVec::from_slice(&deduped[len.saturating_sub(4)..])
    } else {
        deduped
    }
}

fn apply_fallback_prefixes(
    result_v4: &mut IpRange<Ipv4Net>,
    result_v6: &mut IpRange<Ipv6Net>,
    announced_v4: &IpRange<Ipv4Net>,
    announced_v6: &IpRange<Ipv6Net>,
    fallback_prefixes: &[IpNet],
) {
    let mut fallback_v4 = IpRange::new();
    let mut fallback_v6 = IpRange::new();
    for prefix in fallback_prefixes {
        match prefix {
            IpNet::V4(net) => {
                fallback_v4.add(*net);
            }
            IpNet::V6(net) => {
                fallback_v6.add(*net);
            }
        }
    }

    for net in fallback_v4.exclude(announced_v4).iter() {
        result_v4.add(net);
    }
    for net in fallback_v6.exclude(announced_v6).iter() {
        result_v6.add(net);
    }
}

fn emit_sorted<N>(range: &IpRange<N>)
where
    N: IpRangeNet + ToNetwork<N> + Clone + Ord + std::fmt::Display,
{
    let mut nets: Vec<N> = range.iter().collect();
    nets.sort_unstable();
    for net in nets {
        println!("{}", net);
    }
}

#[derive(Serialize, Deserialize)]
struct AsnData {
    v4: AsnRangesV4,
    v6: AsnRangesV6,
    announced_v4: IpRange<Ipv4Net>,
    announced_v6: IpRange<Ipv6Net>,
    direct_upstreams: DirectUpstreams,
}

#[derive(Serialize, Deserialize)]
struct CachedRanges {
    version: u8,
    ignore_private_asn: bool,
    origin_only: bool,
    domestic_policy_fingerprint: Option<u64>,
    data: AsnData,
}

fn cache_path(
    mrt_files: &[PathBuf],
    ignore_private_asn: bool,
    origin_only: bool,
    domestic_policy_fingerprint: Option<u64>,
) -> PathBuf {
    let mut sources: Vec<String> = mrt_files
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    sources.sort();

    let mut hasher = DefaultHasher::new();
    CACHE_FORMAT_VERSION.hash(&mut hasher);
    ignore_private_asn.hash(&mut hasher);
    origin_only.hash(&mut hasher);
    domestic_policy_fingerprint.hash(&mut hasher);
    sources.hash(&mut hasher);
    let hash = hasher.finish();
    PathBuf::from(format!("cache-{hash:016x}.bin"))
}

fn load_cache(
    path: &Path,
    ignore_private_asn: bool,
    origin_only: bool,
    domestic_policy_fingerprint: Option<u64>,
) -> Option<AsnData> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    let cache: CachedRanges = bincode::deserialize_from(reader).ok()?;
    if cache.version == CACHE_FORMAT_VERSION
        && cache.ignore_private_asn == ignore_private_asn
        && cache.origin_only == origin_only
        && cache.domestic_policy_fingerprint == domestic_policy_fingerprint
    {
        Some(cache.data)
    } else {
        None
    }
}

fn save_cache(path: &Path, cache: CachedRanges) -> CachedRanges {
    if let Ok(file) = File::create(path) {
        let writer = BufWriter::new(file);
        let _ = bincode::serialize_into(writer, &cache);
    }
    cache
}

fn build_asn_data(
    mrt_files: &[PathBuf],
    ignore_private_asn: bool,
    origin_only: bool,
    domestic_policy: Option<&DomesticPolicy>,
) -> AsnData {
    // Step 1: parse each MRT file in parallel
    let parsed: Vec<ParsedMrtData> = mrt_files
        .par_iter()
        .map(|mrt_file| {
            process_mrt_file(
                mrt_file.as_path(),
                ignore_private_asn,
                !origin_only,
                domestic_policy,
            )
        })
        .collect();
    let domestic_origins: HashSet<u32> = parsed
        .iter()
        .flat_map(|data| data.domestic_origins.iter().copied())
        .collect();

    // Step 2: merge prefix maps, announced ranges, and split points
    let mut prefix_map_v4: PrefixMap<Ipv4Net, VecSet<[u32; 4]>> = PrefixMap::new();
    let mut prefix_map_v6: PrefixMap<Ipv6Net, VecSet<[u32; 4]>> = PrefixMap::new();
    let mut announced_v4: IpRange<Ipv4Net> = IpRange::new();
    let mut announced_v6: IpRange<Ipv6Net> = IpRange::new();
    let mut as_paths_v4: HashMap<Ipv4Net, HashMap<u32, Vec<SmallVec<[u32; 4]>>>> = HashMap::new();
    let mut as_paths_v6: HashMap<Ipv6Net, HashMap<u32, Vec<SmallVec<[u32; 4]>>>> = HashMap::new();
    let mut direct_upstreams: DirectUpstreams = HashMap::new();
    let mut split_points_v4_set: BTreeSet<Ipv4Addr> = BTreeSet::new();
    let mut split_points_v6_set: BTreeSet<Ipv6Addr> = BTreeSet::new();

    for data in parsed {
        for net in data.announced_v4.iter() {
            announced_v4.add(net);
        }
        for net in data.announced_v6.iter() {
            announced_v6.add(net);
        }
        for (net, asns) in data.prefix_map_v4 {
            let entry = prefix_map_v4.entry(net).or_default();
            if domestic_policy.is_some() {
                entry.extend(
                    asns.into_iter()
                        .filter(|asn| domestic_origins.contains(asn)),
                );
            } else {
                entry.extend(asns);
            }
        }
        for (net, asns) in data.prefix_map_v6 {
            let entry = prefix_map_v6.entry(net).or_default();
            if domestic_policy.is_some() {
                entry.extend(
                    asns.into_iter()
                        .filter(|asn| domestic_origins.contains(asn)),
                );
            } else {
                entry.extend(asns);
            }
        }
        for (net, origins) in data.as_paths_v4 {
            let entry = as_paths_v4.entry(net).or_default();
            for (origin, paths) in origins {
                entry.entry(origin).or_default().extend(paths);
            }
        }
        for (net, origins) in data.as_paths_v6 {
            let entry = as_paths_v6.entry(net).or_default();
            for (origin, paths) in origins {
                entry.entry(origin).or_default().extend(paths);
            }
        }
        for (origin, upstreams) in data.direct_upstreams {
            direct_upstreams
                .entry(origin)
                .or_default()
                .extend(upstreams);
        }
        split_points_v4_set.extend(data.split_points_v4);
        split_points_v6_set.extend(data.split_points_v6);
    }

    // Step 3: Sort split points (BTreeSet already keeps them sorted)
    let split_points_v4: Vec<Ipv4Addr> = split_points_v4_set.into_iter().collect();
    let split_points_v6: Vec<Ipv6Addr> = split_points_v6_set.into_iter().collect();

    if !origin_only {
        // Incorporate shared upstream ASNs (longest common suffix) across all MRT files
        for (net, origins) in as_paths_v4 {
            let entry = prefix_map_v4.entry(net).or_default();
            for (_origin, paths) in origins {
                let shared_upstreams = longest_common_suffix(&paths);
                entry.extend(shared_upstreams);
            }
        }

        for (net, origins) in as_paths_v6 {
            let entry = prefix_map_v6.entry(net).or_default();
            for (_origin, paths) in origins {
                let shared_upstreams = longest_common_suffix(&paths);
                entry.extend(shared_upstreams);
            }
        }
    }

    // Step 4: Build origin-AS to IP range mapping
    let mut asn_ranges_v4: AsnRangesV4 = HashMap::new();
    let mut asn_ranges_v6: AsnRangesV6 = HashMap::new();

    // Process IPv4 split points
    for i in 0..split_points_v4.len().saturating_sub(1) {
        let start = split_points_v4[i];
        let end = split_points_v4[i + 1];

        // Look up origin ASNs at this exact address using longest prefix match
        let lookup_prefix = Ipv4Net::new(start, 32).unwrap();
        if let Some((_, asns)) = prefix_map_v4.get_lpm(&lookup_prefix) {
            // For each origin ASN, add this interval
            for &asn in asns {
                // Convert interval [start, end) to CIDR ranges
                let nets = interval_to_cidrs_v4(start, end);
                let range = asn_ranges_v4.entry(asn).or_insert_with(IpRange::new);
                for net in nets {
                    range.add(net);
                }
            }
        }
    }

    // Process IPv6 split points
    for i in 0..split_points_v6.len().saturating_sub(1) {
        let start = split_points_v6[i];
        let end = split_points_v6[i + 1];

        // Look up origin ASNs at this exact address using longest prefix match
        let lookup_prefix = Ipv6Net::new(start, 128).unwrap();
        if let Some((_, asns)) = prefix_map_v6.get_lpm(&lookup_prefix) {
            // For each origin ASN, add this interval
            for &asn in asns {
                // Convert interval [start, end) to CIDR ranges
                let nets = interval_to_cidrs_v6(start, end);
                let range = asn_ranges_v6.entry(asn).or_insert_with(IpRange::new);
                for net in nets {
                    range.add(net);
                }
            }
        }
    }

    announced_v4.simplify();
    announced_v6.simplify();

    AsnData {
        v4: asn_ranges_v4,
        v6: asn_ranges_v6,
        announced_v4,
        announced_v6,
        direct_upstreams,
    }
}

fn has_domestic_suffix(path: &[u32], domestic_policy: &DomesticPolicy) -> bool {
    path.iter()
        .rev()
        .take_while(|asn| domestic_policy.asn_countries.get(asn).map(String::as_str) == Some("CN"))
        .any(|asn| domestic_policy.trusted_transit_asns.contains(asn))
}

fn domestic_policy_fingerprint(domestic_policy: &DomesticPolicy) -> u64 {
    let mut trusted: Vec<u32> = domestic_policy
        .trusted_transit_asns
        .iter()
        .copied()
        .collect();
    trusted.sort_unstable();
    let mut countries: Vec<(&u32, &String)> = domestic_policy.asn_countries.iter().collect();
    countries.sort_unstable_by_key(|(asn, _)| **asn);

    let mut hasher = DefaultHasher::new();
    trusted.hash(&mut hasher);
    countries.hash(&mut hasher);
    hasher.finish()
}

fn process_mrt_file(
    mrt_file: &Path,
    ignore_private_asn: bool,
    collect_as_paths: bool,
    domestic_policy: Option<&DomesticPolicy>,
) -> ParsedMrtData {
    let rib_path = mrt_file.to_string_lossy().into_owned();
    let parser = BgpkitParser::new(rib_path.as_str())
        .unwrap_or_else(|_| panic!("failed to open MRT/RIB file {rib_path} with bgpkit"));

    let mut prefix_map_v4: PrefixMap<Ipv4Net, VecSet<[u32; 4]>> = PrefixMap::new();
    let mut prefix_map_v6: PrefixMap<Ipv6Net, VecSet<[u32; 4]>> = PrefixMap::new();
    let mut announced_v4: IpRange<Ipv4Net> = IpRange::new();
    let mut announced_v6: IpRange<Ipv6Net> = IpRange::new();
    let mut as_paths_v4: HashMap<Ipv4Net, HashMap<u32, Vec<SmallVec<[u32; 4]>>>> = HashMap::new();
    let mut as_paths_v6: HashMap<Ipv6Net, HashMap<u32, Vec<SmallVec<[u32; 4]>>>> = HashMap::new();
    let mut domestic_origins: HashSet<u32> = HashSet::new();
    let mut direct_upstreams: DirectUpstreams = HashMap::new();
    let mut split_points_v4: BTreeSet<Ipv4Addr> = BTreeSet::new();
    let mut split_points_v6: BTreeSet<Ipv6Addr> = BTreeSet::new();

    for elem in parser.into_elem_iter() {
        if !matches!(elem.elem_type, ElemType::ANNOUNCE) {
            continue;
        }

        if elem.prefix.prefix.prefix_len() > 0 {
            match &elem.prefix.prefix {
                IpNet::V4(net) => {
                    announced_v4.add(*net);
                }
                IpNet::V6(net) => {
                    announced_v6.add(*net);
                }
            }
        }

        let origins = match &elem.origin_asns {
            Some(origins) => origins,
            None => continue,
        };

        if ignore_private_asn && origins.iter().any(|asn| is_private_asn(asn.to_u32())) {
            continue;
        }

        let origin_asns: HashSet<u32> = origins.iter().map(|asn| asn.to_u32()).collect();
        let full_as_path: Option<Vec<u32>> = elem
            .as_path
            .as_ref()
            .and_then(|path| path.to_u32_vec_opt(false))
            .map(|path| {
                path.into_iter().fold(Vec::new(), |mut normalized, asn| {
                    if normalized.last().copied() != Some(asn) {
                        normalized.push(asn);
                    }
                    normalized
                })
            });
        let as_path = full_as_path.as_deref().map(normalize_as_path);
        let has_trusted_cn_transit = match (full_as_path.as_deref(), domestic_policy) {
            (Some(path), Some(policy)) => has_domestic_suffix(path, policy),
            _ => false,
        };
        if has_trusted_cn_transit {
            domestic_origins.extend(origin_asns.iter().copied());
        }

        if let Some(path) = &as_path {
            if let Some(&upstream) = path.iter().rev().nth(1) {
                if !is_private_asn(upstream) {
                    for &origin in &origin_asns {
                        direct_upstreams.entry(origin).or_default().insert(upstream);
                    }
                }
            }
        }

        match elem.prefix.prefix {
            IpNet::V4(net) => {
                prefix_map_v4
                    .entry(net)
                    .or_default()
                    .extend(origin_asns.iter().copied());
                split_points_v4.insert(net.network());
                u32::from(net.broadcast())
                    .checked_add(1)
                    .map(Ipv4Addr::from)
                    .map(|e| split_points_v4.insert(e));

                if collect_as_paths && let Some(path) = &as_path {
                    let entry = as_paths_v4.entry(net).or_default();
                    for &origin in &origin_asns {
                        entry.entry(origin).or_default().push(path.clone());
                    }
                }
            }
            IpNet::V6(net) => {
                prefix_map_v6
                    .entry(net)
                    .or_default()
                    .extend(origin_asns.iter().copied());
                split_points_v6.insert(net.network());
                u128::from(net.broadcast())
                    .checked_add(1)
                    .map(Ipv6Addr::from)
                    .map(|e| split_points_v6.insert(e));

                if collect_as_paths && let Some(path) = &as_path {
                    let entry = as_paths_v6.entry(net).or_default();
                    for &origin in &origin_asns {
                        entry.entry(origin).or_default().push(path.clone());
                    }
                }
            }
        }
    }

    announced_v4.simplify();
    announced_v6.simplify();

    ParsedMrtData {
        prefix_map_v4,
        prefix_map_v6,
        announced_v4,
        announced_v6,
        as_paths_v4,
        as_paths_v6,
        domestic_origins,
        direct_upstreams,
        split_points_v4,
        split_points_v6,
    }
}

fn parse_asn_country(line: &str) -> Option<(u32, &str)> {
    let line = line.trim_end();
    let (asn_part, country) = line.rsplit_once(',')?;
    let country = country.trim();
    let asn = asn_part
        .strip_prefix("AS")?
        .split_whitespace()
        .next()?
        .parse()
        .ok()?;
    if country.len() == 2 {
        Some((asn, country))
    } else {
        None
    }
}

fn load_asn_countries(path: &Path) -> std::io::Result<HashMap<u32, String>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut countries = HashMap::new();
    for line in reader.lines() {
        let line = line?;
        if let Some((asn, country)) = parse_asn_country(&line) {
            countries.insert(asn, country.to_ascii_uppercase());
        }
    }
    Ok(countries)
}

fn load_prefixes(path: &Path) -> std::io::Result<Vec<IpNet>> {
    let file = File::open(path)?;
    BufReader::new(file)
        .lines()
        .map(|line| {
            let line = line?;
            line.trim()
                .parse()
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))
        })
        .collect()
}

fn load_asn_set(path: &Path) -> std::io::Result<HashSet<u32>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    reader
        .lines()
        .map(|line| {
            line?
                .trim()
                .parse()
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))
        })
        .collect()
}

fn foreign_upstream_only_asns(
    target_asns: &HashSet<u32>,
    direct_upstreams: &DirectUpstreams,
    asn_countries: &HashMap<u32, String>,
    country: &str,
) -> Vec<u32> {
    let country = country.to_ascii_uppercase();
    let mut matches: Vec<u32> = target_asns
        .iter()
        .copied()
        .filter(|asn| {
            let Some(upstreams) = direct_upstreams.get(asn) else {
                return false;
            };
            !upstreams.is_empty()
                && upstreams.iter().all(|upstream| {
                    matches!(asn_countries.get(upstream), Some(upstream_country) if upstream_country != &country)
                })
        })
        .collect();
    matches.sort_unstable();
    matches
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_interval_to_cidrs_v4_simple() {
        let start = Ipv4Addr::from_str("192.168.0.0").unwrap();
        let end = Ipv4Addr::from_str("192.168.1.0").unwrap();
        let cidrs = interval_to_cidrs_v4(start, end);
        assert_eq!(cidrs.len(), 1);
        assert_eq!(cidrs[0], Ipv4Net::from_str("192.168.0.0/24").unwrap());
    }

    #[test]
    fn test_interval_to_cidrs_v4_complex() {
        // [10.0.0.0, 10.0.2.0) should produce 10.0.0.0/23
        let start = Ipv4Addr::from_str("10.0.0.0").unwrap();
        let end = Ipv4Addr::from_str("10.0.2.0").unwrap();
        let cidrs = interval_to_cidrs_v4(start, end);
        assert_eq!(cidrs.len(), 1);
        assert_eq!(cidrs[0], Ipv4Net::from_str("10.0.0.0/23").unwrap());
    }

    #[test]
    fn test_interval_to_cidrs_v4_unaligned() {
        // [10.0.1.0, 10.0.2.0) should produce 10.0.1.0/24
        let start = Ipv4Addr::from_str("10.0.1.0").unwrap();
        let end = Ipv4Addr::from_str("10.0.2.0").unwrap();
        let cidrs = interval_to_cidrs_v4(start, end);
        assert_eq!(cidrs.len(), 1);
        assert_eq!(cidrs[0], Ipv4Net::from_str("10.0.1.0/24").unwrap());
    }

    #[test]
    fn test_interval_to_cidrs_v4_multiple() {
        // [10.0.1.0, 10.0.3.0) should produce 10.0.1.0/24, 10.0.2.0/24
        let start = Ipv4Addr::from_str("10.0.1.0").unwrap();
        let end = Ipv4Addr::from_str("10.0.3.0").unwrap();
        let cidrs = interval_to_cidrs_v4(start, end);
        assert_eq!(cidrs.len(), 2);
        assert!(cidrs.contains(&Ipv4Net::from_str("10.0.1.0/24").unwrap()));
        assert!(cidrs.contains(&Ipv4Net::from_str("10.0.2.0/24").unwrap()));
    }

    #[test]
    fn detects_private_asn_ranges() {
        assert!(is_private_asn(64512));
        assert!(is_private_asn(65534));
        assert!(is_private_asn(4_200_000_000));
        assert!(is_private_asn(4_294_967_294));
        assert!(!is_private_asn(64511));
        assert!(!is_private_asn(13335));
    }

    #[test]
    fn computes_longest_common_suffix() {
        let paths = vec![
            SmallVec::from_vec(vec![1, 64512, 13335, 15169]),
            SmallVec::from_vec(vec![64500, 64512, 13335, 15169]),
            SmallVec::from_vec(vec![64501, 9999, 13335, 15169]),
        ];
        assert_eq!(longest_common_suffix(&paths).as_slice(), &[13335, 15169]);

        // limited to the last 4 elements
        let long_paths = vec![SmallVec::from_vec(vec![10, 20, 30, 40, 50, 60])];
        assert_eq!(
            longest_common_suffix(&long_paths).as_slice(),
            &[30, 40, 50, 60]
        );
    }

    #[test]
    fn normalizes_as_path_before_upstream_detection() {
        assert_eq!(
            normalize_as_path(&[2914, 20473, 139589, 139589]).as_slice(),
            &[2914, 20473, 139589]
        );
        assert_eq!(
            normalize_as_path(&[1, 2, 3, 4, 5, 5]).as_slice(),
            &[2, 3, 4, 5]
        );
    }

    #[test]
    fn cache_path_changes_when_origin_only_changes() {
        let mrt_files = vec![PathBuf::from("rib-a.gz"), PathBuf::from("rib-b.gz")];
        assert_ne!(
            cache_path(&mrt_files, false, false, None),
            cache_path(&mrt_files, false, true, None)
        );
    }

    #[test]
    fn cache_path_changes_with_domestic_policy() {
        let mrt_files = vec![PathBuf::from("rib-a.gz")];
        assert_ne!(
            cache_path(&mrt_files, false, true, None),
            cache_path(&mrt_files, false, true, Some(1))
        );
        assert_ne!(
            cache_path(&mrt_files, false, true, Some(1)),
            cache_path(&mrt_files, false, true, Some(2))
        );
    }

    #[test]
    fn detects_trusted_transit_in_contiguous_cn_suffix() {
        let policy = DomesticPolicy {
            trusted_transit_asns: HashSet::from([4538, 7497]),
            asn_countries: HashMap::from([
                (24489, "CN".to_string()),
                (23911, "CN".to_string()),
                (4538, "CN".to_string()),
                (38345, "CN".to_string()),
                (7497, "CN".to_string()),
                (6939, "US".to_string()),
            ]),
        };

        assert!(has_domestic_suffix(&[6939, 4538, 23911, 24489], &policy));
        assert!(has_domestic_suffix(&[6939, 7497, 38345], &policy));
        assert!(!has_domestic_suffix(&[4538, 6939, 24489], &policy));
        assert!(!has_domestic_suffix(&[4538, 64500, 24489], &policy));
    }

    #[test]
    fn trusted_transit_filter_requires_origin_only_and_country_data() {
        assert!(
            Opts::try_parse_from([
                "china-operator-ip",
                "--asn-country-file",
                "countries.txt",
                "--trusted-cn-transit-file",
                "trusted.txt",
                "1",
            ])
            .is_err()
        );
        assert!(
            Opts::try_parse_from([
                "china-operator-ip",
                "--origin-only",
                "--trusted-cn-transit-file",
                "trusted.txt",
                "1",
            ])
            .is_err()
        );
        assert!(
            Opts::try_parse_from([
                "china-operator-ip",
                "--origin-only",
                "--asn-country-file",
                "countries.txt",
                "--trusted-cn-transit-file",
                "trusted.txt",
                "1",
            ])
            .is_ok()
        );
    }

    #[test]
    fn empty_filtered_prefix_blocks_aggregate_fallback() {
        let mut prefixes: PrefixMap<Ipv4Net, VecSet<[u32; 4]>> = PrefixMap::new();
        prefixes
            .entry("10.0.0.0/8".parse().unwrap())
            .or_default()
            .extend([4134]);
        prefixes.entry("10.1.2.0/24".parse().unwrap()).or_default();

        let lookup = "10.1.2.1/32".parse().unwrap();
        let (_, origins) = prefixes.get_lpm(&lookup).unwrap();
        assert!(origins.is_empty());
    }

    #[test]
    fn fallback_completes_an_unannounced_half() {
        let mut result_v4 = IpRange::new();
        result_v4.add("121.46.0.0/19".parse().unwrap());
        let mut announced_v4 = IpRange::new();
        announced_v4.add("121.46.0.0/19".parse().unwrap());

        apply_fallback_prefixes(
            &mut result_v4,
            &mut IpRange::new(),
            &announced_v4,
            &IpRange::new(),
            &["121.46.0.0/18".parse().unwrap()],
        );
        result_v4.simplify();

        assert_eq!(
            result_v4
                .iter()
                .map(|net| net.to_string())
                .collect::<Vec<_>>(),
            ["121.46.0.0/18"]
        );
    }

    #[test]
    fn fallback_preserves_a_more_specific_announced_hole() {
        let mut result_v4 = IpRange::new();
        let mut announced_v4 = IpRange::new();
        announced_v4.add("10.0.0.64/26".parse().unwrap());

        apply_fallback_prefixes(
            &mut result_v4,
            &mut IpRange::new(),
            &announced_v4,
            &IpRange::new(),
            &["10.0.0.0/24".parse().unwrap()],
        );
        result_v4.simplify();

        let mut actual = result_v4
            .iter()
            .map(|net| net.to_string())
            .collect::<Vec<_>>();
        actual.sort();
        assert_eq!(actual, ["10.0.0.0/26", "10.0.0.128/25"]);
    }

    #[test]
    fn fallback_adds_nothing_when_fully_announced() {
        let mut result_v4 = IpRange::new();
        let mut announced_v4 = IpRange::new();
        announced_v4.add("10.0.0.0/24".parse().unwrap());

        apply_fallback_prefixes(
            &mut result_v4,
            &mut IpRange::new(),
            &announced_v4,
            &IpRange::new(),
            &["10.0.0.0/24".parse().unwrap()],
        );

        assert!(result_v4.is_empty());
    }

    #[test]
    fn fallback_supports_ipv6() {
        let mut result_v6 = IpRange::new();
        result_v6.add("2001:db8::/33".parse().unwrap());
        let mut announced_v6 = IpRange::new();
        announced_v6.add("2001:db8::/33".parse().unwrap());

        apply_fallback_prefixes(
            &mut IpRange::new(),
            &mut result_v6,
            &IpRange::new(),
            &announced_v6,
            &["2001:db8::/32".parse().unwrap()],
        );
        result_v6.simplify();

        assert_eq!(
            result_v6
                .iter()
                .map(|net| net.to_string())
                .collect::<Vec<_>>(),
            ["2001:db8::/32"]
        );
    }

    #[test]
    fn accepts_fallback_prefix_file() {
        let opts = Opts::try_parse_from([
            "china-operator-ip",
            "--fallback-prefix-file",
            "prefixes.txt",
            "4134",
        ])
        .unwrap();

        assert_eq!(
            opts.fallback_prefix_file,
            Some(PathBuf::from("prefixes.txt"))
        );
    }

    #[test]
    fn parses_asn_country_lines() {
        assert_eq!(
            parse_asn_country("AS4134 CHINANET-BACKBONE, CN"),
            Some((4134, "CN"))
        );
        assert_eq!(parse_asn_country("AS15169 GOOGLE, US"), Some((15169, "US")));
        assert_eq!(parse_asn_country("invalid"), None);
    }

    #[test]
    fn detects_foreign_upstream_only_asns() {
        let target_asns = HashSet::from([1, 2, 3, 4]);
        let direct_upstreams = HashMap::from([
            (1, BTreeSet::from([100, 101])),
            (2, BTreeSet::from([100, 102])),
            (3, BTreeSet::from([200])),
            (4, BTreeSet::new()),
        ]);
        let asn_countries = HashMap::from([
            (100, "US".to_string()),
            (101, "JP".to_string()),
            (102, "CN".to_string()),
        ]);

        assert_eq!(
            foreign_upstream_only_asns(&target_asns, &direct_upstreams, &asn_countries, "CN"),
            vec![1]
        );
    }
}
