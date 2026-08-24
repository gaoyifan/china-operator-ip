use crate::asn::{AsPath, Asn, longest_common_suffix};
use ipnet::{IpNet, Ipv4Net, Ipv4Subnets, Ipv6Net, Ipv6Subnets};
use iprange::{IpNet as IpRangeNet, IpRange};
use prefix_trie::{Prefix, PrefixMap};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt::Display;
use std::hash::Hash;
use std::net::{Ipv4Addr, Ipv6Addr};
use vec_collections::VecSet;

type Origins = VecSet<[Asn; 4]>;
type AsnRangeMap<N> = HashMap<Asn, IpRange<N>>;

#[derive(Default, Deserialize, Serialize)]
pub(crate) struct DualStack<V4, V6> {
    pub(crate) v4: V4,
    pub(crate) v6: V6,
}

pub(crate) type IpRanges = DualStack<IpRange<Ipv4Net>, IpRange<Ipv6Net>>;
pub(crate) type AsnRanges = DualStack<AsnRangeMap<Ipv4Net>, AsnRangeMap<Ipv6Net>>;

impl IpRanges {
    pub(crate) fn add_prefix(&mut self, prefix: IpNet) {
        match prefix {
            IpNet::V4(net) => {
                self.v4.add(net);
            }
            IpNet::V6(net) => {
                self.v6.add(net);
            }
        }
    }

    pub(crate) fn add_unannounced(&mut self, announced: &Self, prefixes: &[IpNet]) {
        let mut fallback = Self::default();
        for prefix in prefixes {
            fallback.add_prefix(*prefix);
        }
        fallback.simplify();

        for net in fallback.v4.exclude(&announced.v4).iter() {
            self.v4.add(net);
        }
        for net in fallback.v6.exclude(&announced.v6).iter() {
            self.v6.add(net);
        }
    }

    pub(crate) fn simplify(&mut self) {
        self.v4.simplify();
        self.v6.simplify();
    }

    pub(crate) fn lines(&self) -> Vec<String> {
        fn sorted<N>(range: &IpRange<N>) -> Vec<String>
        where
            N: IpRangeNet + Clone + Display,
        {
            let mut nets: Vec<N> = range.iter().collect();
            nets.sort_unstable();
            nets.into_iter().map(|net| net.to_string()).collect()
        }

        let mut lines = sorted(&self.v4);
        lines.extend(sorted(&self.v6));
        lines
    }
}

impl AsnRanges {
    pub(crate) fn contains(&self, asn: &Asn) -> bool {
        self.v4.contains_key(asn) || self.v6.contains_key(asn)
    }

    pub(crate) fn select(&self, targets: &HashSet<Asn>, excluded: &HashSet<Asn>) -> IpRanges {
        fn add_selected<N>(
            result: &mut IpRange<N>,
            ranges: &AsnRangeMap<N>,
            targets: &HashSet<Asn>,
            excluded: &HashSet<Asn>,
        ) where
            N: IpRangeNet + Clone,
        {
            for asn in targets.difference(excluded) {
                if let Some(range) = ranges.get(asn) {
                    for net in range.iter() {
                        result.add(net);
                    }
                }
            }
        }

        let mut result = IpRanges::default();
        add_selected(&mut result.v4, &self.v4, targets, excluded);
        add_selected(&mut result.v6, &self.v6, targets, excluded);
        result
    }
}

trait AddressFamily: Prefix + IpRangeNet + Hash {
    type Addr: Copy + Ord;

    fn host(address: Self::Addr) -> Self;
    fn network(&self) -> Self::Addr;
    fn next_after(&self) -> Option<Self::Addr>;
    fn interval_to_cidrs(start: Self::Addr, end: Self::Addr) -> Vec<Self>;
}

impl AddressFamily for Ipv4Net {
    type Addr = Ipv4Addr;

    fn host(address: Self::Addr) -> Self {
        Self::new(address, 32).unwrap()
    }

    fn network(&self) -> Self::Addr {
        self.network()
    }

    fn next_after(&self) -> Option<Self::Addr> {
        u32::from(self.broadcast())
            .checked_add(1)
            .map(Ipv4Addr::from)
    }

    fn interval_to_cidrs(start: Self::Addr, end: Self::Addr) -> Vec<Self> {
        if start >= end {
            return Vec::new();
        }
        let end_inclusive = u32::from(end).saturating_sub(1);
        Ipv4Subnets::new(start, Ipv4Addr::from(end_inclusive), 0).collect()
    }
}

impl AddressFamily for Ipv6Net {
    type Addr = Ipv6Addr;

    fn host(address: Self::Addr) -> Self {
        Self::new(address, 128).unwrap()
    }

    fn network(&self) -> Self::Addr {
        self.network()
    }

    fn next_after(&self) -> Option<Self::Addr> {
        u128::from(self.broadcast())
            .checked_add(1)
            .map(Ipv6Addr::from)
    }

    fn interval_to_cidrs(start: Self::Addr, end: Self::Addr) -> Vec<Self> {
        if start >= end {
            return Vec::new();
        }
        let end_inclusive = u128::from(end).saturating_sub(1);
        Ipv6Subnets::new(start, Ipv6Addr::from(end_inclusive), 0).collect()
    }
}

struct FamilyTable<N: AddressFamily> {
    prefixes: PrefixMap<N, Origins>,
    announced: IpRange<N>,
    paths: HashMap<N, HashMap<Asn, Vec<AsPath>>>,
    split_points: BTreeSet<N::Addr>,
}

impl<N: AddressFamily> Default for FamilyTable<N> {
    fn default() -> Self {
        Self {
            prefixes: PrefixMap::new(),
            announced: IpRange::new(),
            paths: HashMap::new(),
            split_points: BTreeSet::new(),
        }
    }
}

impl<N: AddressFamily> FamilyTable<N> {
    fn announce(&mut self, net: N) {
        if Prefix::prefix_len(&net) > 0 {
            self.announced.add(net);
        }
    }

    fn classify(
        &mut self,
        net: N,
        origins: &HashSet<Asn>,
        path: Option<&AsPath>,
        collect_paths: bool,
    ) {
        self.prefixes
            .entry(net)
            .or_insert_with(Origins::empty)
            .extend(origins.iter().copied());
        self.split_points.insert(net.network());
        if let Some(end) = net.next_after() {
            self.split_points.insert(end);
        }

        if collect_paths && let Some(path) = path {
            let entry = self.paths.entry(net).or_default();
            for origin in origins {
                entry.entry(*origin).or_default().push(path.clone());
            }
        }
    }

    fn merge(&mut self, other: Self, allowed_origins: Option<&HashSet<Asn>>) {
        for net in other.announced.iter() {
            self.announced.add(net);
        }
        for (net, origins) in other.prefixes {
            let entry = self.prefixes.entry(net).or_insert_with(Origins::empty);
            match allowed_origins {
                Some(allowed) => {
                    entry.extend(origins.into_iter().filter(|asn| allowed.contains(asn)))
                }
                None => entry.extend(origins),
            }
        }
        for (net, origins) in other.paths {
            let entry = self.paths.entry(net).or_default();
            for (origin, paths) in origins {
                entry.entry(origin).or_default().extend(paths);
            }
        }
        self.split_points.extend(other.split_points);
    }

    fn add_shared_upstreams(&mut self) {
        for (net, origins) in std::mem::take(&mut self.paths) {
            let entry = self.prefixes.entry(net).or_insert_with(Origins::empty);
            for paths in origins.into_values() {
                entry.extend(longest_common_suffix(&paths));
            }
        }
    }

    fn into_ranges(mut self) -> (AsnRangeMap<N>, IpRange<N>) {
        let points: Vec<N::Addr> = self.split_points.into_iter().collect();
        let mut ranges = HashMap::new();

        for window in points.windows(2) {
            let start = window[0];
            let end = window[1];
            let Some((_, origins)) = self.prefixes.get_lpm(&N::host(start)) else {
                continue;
            };
            let cidrs = N::interval_to_cidrs(start, end);
            for origin in origins {
                let range: &mut IpRange<N> = ranges.entry(*origin).or_default();
                for net in &cidrs {
                    range.add(*net);
                }
            }
        }

        self.announced.simplify();
        (ranges, self.announced)
    }
}

#[derive(Default)]
pub(crate) struct MrtTables {
    families: DualStack<FamilyTable<Ipv4Net>, FamilyTable<Ipv6Net>>,
}

impl MrtTables {
    pub(crate) fn announce(&mut self, prefix: &IpNet) {
        match prefix {
            IpNet::V4(net) => self.families.v4.announce(*net),
            IpNet::V6(net) => self.families.v6.announce(*net),
        }
    }

    pub(crate) fn classify(
        &mut self,
        prefix: IpNet,
        origins: &HashSet<Asn>,
        path: Option<&AsPath>,
        collect_paths: bool,
    ) {
        match prefix {
            IpNet::V4(net) => self.families.v4.classify(net, origins, path, collect_paths),
            IpNet::V6(net) => self.families.v6.classify(net, origins, path, collect_paths),
        }
    }

    pub(crate) fn merge(&mut self, other: Self, allowed_origins: Option<&HashSet<Asn>>) {
        self.families.v4.merge(other.families.v4, allowed_origins);
        self.families.v6.merge(other.families.v6, allowed_origins);
    }

    pub(crate) fn add_shared_upstreams(&mut self) {
        self.families.v4.add_shared_upstreams();
        self.families.v6.add_shared_upstreams();
    }

    pub(crate) fn into_ranges(self) -> (AsnRanges, IpRanges) {
        let (ranges_v4, announced_v4) = self.families.v4.into_ranges();
        let (ranges_v6, announced_v6) = self.families.v6.into_ranges();
        (
            AsnRanges {
                v4: ranges_v4,
                v6: ranges_v6,
            },
            IpRanges {
                v4: announced_v4,
                v6: announced_v6,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn converts_ipv4_interval_to_one_cidr() {
        let cidrs = Ipv4Net::interval_to_cidrs(
            Ipv4Addr::from_str("192.168.0.0").unwrap(),
            Ipv4Addr::from_str("192.168.1.0").unwrap(),
        );
        assert_eq!(cidrs, [Ipv4Net::from_str("192.168.0.0/24").unwrap()]);
    }

    #[test]
    fn converts_complex_ipv4_interval() {
        let cidrs = Ipv4Net::interval_to_cidrs(
            Ipv4Addr::from_str("10.0.0.0").unwrap(),
            Ipv4Addr::from_str("10.0.2.0").unwrap(),
        );
        assert_eq!(cidrs, [Ipv4Net::from_str("10.0.0.0/23").unwrap()]);
    }

    #[test]
    fn converts_unaligned_ipv4_interval() {
        let cidrs = Ipv4Net::interval_to_cidrs(
            Ipv4Addr::from_str("10.0.1.0").unwrap(),
            Ipv4Addr::from_str("10.0.2.0").unwrap(),
        );
        assert_eq!(cidrs, [Ipv4Net::from_str("10.0.1.0/24").unwrap()]);
    }

    #[test]
    fn converts_ipv4_interval_to_multiple_cidrs() {
        let cidrs = Ipv4Net::interval_to_cidrs(
            Ipv4Addr::from_str("10.0.1.0").unwrap(),
            Ipv4Addr::from_str("10.0.3.0").unwrap(),
        );
        assert_eq!(
            cidrs,
            [
                Ipv4Net::from_str("10.0.1.0/24").unwrap(),
                Ipv4Net::from_str("10.0.2.0/24").unwrap(),
            ]
        );
    }

    #[test]
    fn empty_filtered_prefix_blocks_aggregate_fallback() {
        let mut table = FamilyTable::<Ipv4Net>::default();
        table
            .prefixes
            .entry("10.0.0.0/8".parse().unwrap())
            .or_insert_with(Origins::empty)
            .extend([4134.into()]);
        table
            .prefixes
            .entry("10.1.2.0/24".parse().unwrap())
            .or_insert_with(Origins::empty);

        let (_, origins) = table
            .prefixes
            .get_lpm(&"10.1.2.1/32".parse().unwrap())
            .unwrap();
        assert!(origins.is_empty());
    }

    fn fallback_result(announced: &[&str], fallbacks: &[&str]) -> IpRanges {
        let mut announced_ranges = IpRanges::default();
        for prefix in announced {
            announced_ranges.add_prefix(prefix.parse().unwrap());
        }
        let fallback_prefixes: Vec<IpNet> = fallbacks
            .iter()
            .map(|prefix| prefix.parse().unwrap())
            .collect();
        let mut result = IpRanges::default();
        result.add_unannounced(&announced_ranges, &fallback_prefixes);
        result.simplify();
        result
    }

    #[test]
    fn fallback_completes_an_unannounced_half() {
        let mut result = fallback_result(&["121.46.0.0/19"], &["121.46.0.0/18"]);
        result.add_prefix("121.46.0.0/19".parse().unwrap());
        result.simplify();
        assert_eq!(result.lines(), ["121.46.0.0/18"]);
    }

    #[test]
    fn fallback_preserves_a_more_specific_announced_hole() {
        let result = fallback_result(&["10.0.0.64/26"], &["10.0.0.0/24"]);
        assert_eq!(result.lines(), ["10.0.0.0/26", "10.0.0.128/25"]);
    }

    #[test]
    fn fallback_adds_nothing_when_fully_announced() {
        let result = fallback_result(&["10.0.0.0/24"], &["10.0.0.0/24"]);
        assert!(result.lines().is_empty());
    }

    #[test]
    fn fallback_supports_ipv6() {
        let mut result = fallback_result(&["2001:db8::/33"], &["2001:db8::/32"]);
        result.add_prefix("2001:db8::/33".parse().unwrap());
        result.simplify();
        assert_eq!(result.lines(), ["2001:db8::/32"]);
    }
}
