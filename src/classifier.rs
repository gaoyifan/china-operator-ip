use crate::asn::{Asn, DirectUpstreams, DomesticPolicy, normalize_path};
use crate::ip::{AsnRanges, IpRanges, MrtTables};
use bgpkit_parser::{BgpkitParser, models::ElemType};
use ipnet::IpNet;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy)]
pub(crate) struct ClassifierConfig {
    pub(crate) ignore_private_asn: bool,
    pub(crate) origin_only: bool,
}

#[derive(Deserialize, Serialize)]
pub(crate) struct Classification {
    ranges: AsnRanges,
    announced: IpRanges,
    direct_upstreams: DirectUpstreams,
}

impl Classification {
    pub(crate) fn result(
        &self,
        targets: &HashSet<Asn>,
        excluded: &HashSet<Asn>,
        fallback_prefixes: &[IpNet],
    ) -> IpRanges {
        let mut result = self.ranges.select(targets, excluded);
        result.add_unannounced(&self.announced, fallback_prefixes);
        result.simplify();
        result
    }

    pub(crate) fn seen(&self, targets: &HashSet<Asn>) -> Vec<Asn> {
        let mut seen: Vec<Asn> = targets
            .iter()
            .copied()
            .filter(|asn| self.ranges.contains(asn))
            .collect();
        seen.sort_unstable();
        seen
    }

    pub(crate) fn direct_upstreams(&self) -> &DirectUpstreams {
        &self.direct_upstreams
    }
}

struct ParsedMrtData {
    tables: MrtTables,
    domestic_origins: HashSet<Asn>,
    direct_upstreams: DirectUpstreams,
}

pub(crate) fn build(
    mrt_files: &[PathBuf],
    config: ClassifierConfig,
    domestic_policy: Option<&DomesticPolicy>,
    operator_asns: &HashSet<Asn>,
) -> Classification {
    let parsed: Vec<ParsedMrtData> = mrt_files
        .par_iter()
        .map(|mrt_file| process_mrt_file(mrt_file, config, domestic_policy))
        .collect();
    let domestic_origins: HashSet<Asn> = parsed
        .iter()
        .flat_map(|data| data.domestic_origins.iter().copied())
        .collect();

    let mut tables = MrtTables::default();
    let mut direct_upstreams = DirectUpstreams::new();
    for data in parsed {
        tables.merge(data.tables, domestic_policy.map(|_| &domestic_origins));
        for (origin, upstreams) in data.direct_upstreams {
            direct_upstreams
                .entry(origin)
                .or_default()
                .extend(upstreams);
        }
    }

    if !config.origin_only {
        tables.add_shared_upstreams(operator_asns);
    }
    let (ranges, announced) = tables.into_ranges();
    Classification {
        ranges,
        announced,
        direct_upstreams,
    }
}

fn process_mrt_file(
    mrt_file: &Path,
    config: ClassifierConfig,
    domestic_policy: Option<&DomesticPolicy>,
) -> ParsedMrtData {
    let rib_path = mrt_file.to_string_lossy().into_owned();
    let parser = BgpkitParser::new(rib_path.as_str())
        .unwrap_or_else(|_| panic!("failed to open MRT/RIB file {rib_path} with bgpkit"));
    let mut tables = MrtTables::default();
    let mut domestic_origins = HashSet::new();
    let mut direct_upstreams = DirectUpstreams::new();

    for elem in parser.into_elem_iter() {
        if elem.elem_type != ElemType::ANNOUNCE {
            continue;
        }
        tables.announce(&elem.prefix.prefix);

        let Some(origins) = &elem.origin_asns else {
            continue;
        };
        let origin_asns: HashSet<Asn> = origins.iter().map(|asn| Asn::from(asn.to_u32())).collect();
        if config.ignore_private_asn && origin_asns.iter().any(|asn| asn.is_private()) {
            continue;
        }

        let full_path = elem
            .as_path
            .as_ref()
            .and_then(|path| path.to_u32_vec_opt(false))
            .map(|path| path.into_iter().map(Asn::from).collect::<Vec<_>>());
        let path = full_path.as_deref().map(normalize_path);

        if full_path
            .as_deref()
            .zip(domestic_policy)
            .is_some_and(|(path, policy)| policy.has_domestic_suffix(path))
        {
            domestic_origins.extend(origin_asns.iter().copied());
        }

        if let Some(upstream) = path
            .as_ref()
            .and_then(|path| path.iter().rev().nth(1))
            .copied()
            .filter(|asn| !asn.is_private())
        {
            for origin in &origin_asns {
                direct_upstreams
                    .entry(*origin)
                    .or_default()
                    .insert(upstream);
            }
        }

        tables.classify(
            elem.prefix.prefix,
            &origin_asns,
            path.as_ref().filter(|_| !config.origin_only),
        );
    }

    ParsedMrtData {
        tables,
        domestic_origins,
        direct_upstreams,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bgpkit_parser::{
        encoder::MrtRibEncoder,
        models::{AsPath, BgpElem},
    };

    #[test]
    fn rib_classification_stops_at_the_nearest_operator() {
        let origin = 64496;
        let nearest_operator = 64497;
        let transit = 64498;
        let peer = 64499;
        let mut encoder = MrtRibEncoder::new();
        // Synthetic topology with AS prepending; no live routing data is used.
        let elem = BgpElem {
            timestamp: 1.0,
            peer_ip: "192.0.2.1".parse().unwrap(),
            peer_asn: peer.into(),
            prefix: "198.51.100.0/24".parse().unwrap(),
            as_path: Some(AsPath::from_sequence([
                peer,
                transit,
                nearest_operator,
                origin,
                origin,
            ])),
            ..Default::default()
        };
        encoder.process_elem(&elem);
        let path = std::env::temp_dir().join(format!(
            "china-operator-ip-nearest-operator-{}.mrt",
            std::process::id()
        ));
        std::fs::write(&path, encoder.export_bytes()).unwrap();
        let config = ClassifierConfig {
            ignore_private_asn: true,
            origin_only: false,
        };
        let classification = build(
            std::slice::from_ref(&path),
            config,
            None,
            &HashSet::from([transit.into(), nearest_operator.into()]),
        );
        let origin_only = build(
            std::slice::from_ref(&path),
            ClassifierConfig {
                origin_only: true,
                ..config
            },
            None,
            &HashSet::new(),
        );
        std::fs::remove_file(path).unwrap();

        for asn in [origin, nearest_operator] {
            assert_eq!(
                classification
                    .result(&HashSet::from([asn.into()]), &HashSet::new(), &[])
                    .lines(),
                ["198.51.100.0/24"]
            );
        }
        assert!(
            classification
                .result(&HashSet::from([transit.into()]), &HashSet::new(), &[])
                .lines()
                .is_empty()
        );
        assert_eq!(
            origin_only
                .result(&HashSet::from([origin.into()]), &HashSet::new(), &[])
                .lines(),
            ["198.51.100.0/24"]
        );
        assert!(
            origin_only
                .result(
                    &HashSet::from([nearest_operator.into()]),
                    &HashSet::new(),
                    &[]
                )
                .lines()
                .is_empty()
        );
    }

    #[test]
    fn divergent_operators_across_ribs_are_not_shared_upstreams() {
        let origin = 64496;
        let operators = [64497, 64498];
        let mut paths = Vec::new();
        // The second RIB removes the common operator; the third must not restore it.
        for (index, observed_operators) in [&operators[..1], operators.as_slice(), &operators[..1]]
            .into_iter()
            .enumerate()
        {
            let mut encoder = MrtRibEncoder::new();
            for (peer, &operator) in observed_operators.iter().enumerate() {
                encoder.process_elem(&BgpElem {
                    timestamp: 1.0,
                    peer_ip: std::net::Ipv4Addr::new(192, 0, 2, peer as u8 + 1).into(),
                    peer_asn: operator.into(),
                    prefix: "198.51.100.0/24".parse().unwrap(),
                    as_path: Some(AsPath::from_sequence([operator, origin])),
                    ..Default::default()
                });
            }
            let path = std::env::temp_dir().join(format!(
                "china-operator-ip-divergent-operators-{}-{index}.mrt",
                std::process::id()
            ));
            std::fs::write(&path, encoder.export_bytes()).unwrap();
            paths.push(path);
        }
        let operator_asns = operators.into_iter().map(Asn::from).collect();
        let classification = build(
            &paths,
            ClassifierConfig {
                ignore_private_asn: true,
                origin_only: false,
            },
            None,
            &operator_asns,
        );
        for path in paths {
            std::fs::remove_file(path).unwrap();
        }
        assert_eq!(
            classification
                .result(&HashSet::from([origin.into()]), &HashSet::new(), &[])
                .lines(),
            ["198.51.100.0/24"]
        );
        assert!(
            classification
                .result(&operator_asns, &HashSet::new(), &[])
                .lines()
                .is_empty()
        );
    }
}
