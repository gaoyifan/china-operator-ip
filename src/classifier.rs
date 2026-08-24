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
        tables.add_shared_upstreams();
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
            .map(|path| {
                path.into_iter()
                    .map(Asn::from)
                    .fold(Vec::new(), |mut normalized, asn| {
                        if normalized.last() != Some(&asn) {
                            normalized.push(asn);
                        }
                        normalized
                    })
            });
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
            path.as_ref(),
            !config.origin_only,
        );
    }

    ParsedMrtData {
        tables,
        domestic_origins,
        direct_upstreams,
    }
}
