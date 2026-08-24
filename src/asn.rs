use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::collections::{BTreeSet, HashMap, HashSet, hash_map::DefaultHasher};
use std::fmt;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader};
use std::num::ParseIntError;
use std::path::Path;
use std::str::FromStr;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub(crate) struct Asn(u32);

impl Asn {
    pub(crate) fn is_private(self) -> bool {
        (64512..=65534).contains(&self.0) || (4_200_000_000..=4_294_967_294).contains(&self.0)
    }
}

impl From<u32> for Asn {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl FromStr for Asn {
    type Err = ParseIntError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.parse().map(Self)
    }
}

impl fmt::Display for Asn {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

pub(crate) type AsPath = SmallVec<[Asn; 4]>;
pub(crate) type AsnCountries = HashMap<Asn, String>;
pub(crate) type DirectUpstreams = HashMap<Asn, BTreeSet<Asn>>;

pub(crate) fn normalize_path(path: &[Asn]) -> AsPath {
    let mut deduped = AsPath::new();
    for asn in path {
        if deduped.last() != Some(asn) {
            deduped.push(*asn);
        }
    }
    if deduped.len() > 4 {
        let len = deduped.len();
        AsPath::from_slice(&deduped[len - 4..])
    } else {
        deduped
    }
}

pub(crate) fn longest_common_suffix(paths: &[AsPath]) -> AsPath {
    let Some(first) = paths.first() else {
        return AsPath::new();
    };
    let min_len = paths.iter().map(|path| path.len()).min().unwrap().min(4);
    let mut suffix = AsPath::new();

    for offset in 1..=min_len {
        let candidate = first[first.len() - offset];
        if paths
            .iter()
            .all(|path| path[path.len() - offset] == candidate)
        {
            suffix.push(candidate);
        } else {
            break;
        }
    }
    suffix.reverse();
    suffix
}

pub(crate) struct DomesticPolicy {
    trusted_transit_asns: HashSet<Asn>,
    asn_countries: AsnCountries,
}

impl DomesticPolicy {
    pub(crate) fn from_files(trusted_path: &Path, country_path: &Path) -> std::io::Result<Self> {
        let trusted_transit_asns = load_set(trusted_path).map_err(|err| {
            std::io::Error::new(
                err.kind(),
                format!("failed to load trusted CN transit ASNs: {err}"),
            )
        })?;
        let asn_countries = load_countries(country_path).map_err(|err| {
            std::io::Error::new(
                err.kind(),
                format!(
                    "failed to load ASN country data from {}: {err}",
                    country_path.display()
                ),
            )
        })?;
        Ok(Self {
            trusted_transit_asns,
            asn_countries,
        })
    }

    pub(crate) fn has_domestic_suffix(&self, path: &[Asn]) -> bool {
        path.iter()
            .rev()
            .take_while(|asn| self.asn_countries.get(asn).map(String::as_str) == Some("CN"))
            .any(|asn| self.trusted_transit_asns.contains(asn))
    }

    pub(crate) fn fingerprint(&self) -> u64 {
        let mut trusted: Vec<Asn> = self.trusted_transit_asns.iter().copied().collect();
        trusted.sort_unstable();
        let mut countries: Vec<(&Asn, &String)> = self.asn_countries.iter().collect();
        countries.sort_unstable_by_key(|(asn, _)| **asn);

        let mut hasher = DefaultHasher::new();
        trusted.hash(&mut hasher);
        countries.hash(&mut hasher);
        hasher.finish()
    }
}

fn parse_country(line: &str) -> Option<(Asn, &str)> {
    let line = line.trim_end();
    let (asn_part, country) = line.rsplit_once(',')?;
    let country = country.trim();
    let asn = asn_part
        .strip_prefix("AS")?
        .split_whitespace()
        .next()?
        .parse()
        .ok()?;
    (country.len() == 2).then_some((asn, country))
}

pub(crate) fn load_countries(path: &Path) -> std::io::Result<AsnCountries> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut countries = HashMap::new();
    for line in reader.lines() {
        let line = line?;
        if let Some((asn, country)) = parse_country(&line) {
            countries.insert(asn, country.to_ascii_uppercase());
        }
    }
    Ok(countries)
}

fn load_set(path: &Path) -> std::io::Result<HashSet<Asn>> {
    let file = File::open(path)?;
    BufReader::new(file)
        .lines()
        .map(|line| {
            line?
                .trim()
                .parse()
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))
        })
        .collect()
}

pub(crate) fn foreign_upstream_only(
    targets: &HashSet<Asn>,
    direct_upstreams: &DirectUpstreams,
    countries: &AsnCountries,
    country: &str,
) -> Vec<Asn> {
    let country = country.to_ascii_uppercase();
    let mut matches: Vec<Asn> = targets
        .iter()
        .copied()
        .filter(|asn| {
            let Some(upstreams) = direct_upstreams.get(asn) else {
                return false;
            };
            !upstreams.is_empty()
                && upstreams.iter().all(|upstream| {
                    matches!(countries.get(upstream), Some(upstream_country) if upstream_country != &country)
                })
        })
        .collect();
    matches.sort_unstable();
    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_private_asn_ranges() {
        assert!(Asn::from(64512).is_private());
        assert!(Asn::from(65534).is_private());
        assert!(Asn::from(4_200_000_000).is_private());
        assert!(Asn::from(4_294_967_294).is_private());
        assert!(!Asn::from(64511).is_private());
        assert!(!Asn::from(13335).is_private());
    }

    #[test]
    fn computes_longest_common_suffix() {
        let paths = vec![
            [1, 64512, 13335, 15169]
                .map(Asn::from)
                .into_iter()
                .collect(),
            [64500, 64512, 13335, 15169]
                .map(Asn::from)
                .into_iter()
                .collect(),
            [64501, 9999, 13335, 15169]
                .map(Asn::from)
                .into_iter()
                .collect(),
        ];
        assert_eq!(
            longest_common_suffix(&paths).as_slice(),
            &[Asn::from(13335), Asn::from(15169)]
        );

        let long_paths = vec![
            [10, 20, 30, 40, 50, 60]
                .map(Asn::from)
                .into_iter()
                .collect(),
        ];
        assert_eq!(
            longest_common_suffix(&long_paths).as_slice(),
            &[Asn::from(30), Asn::from(40), Asn::from(50), Asn::from(60),]
        );
    }

    #[test]
    fn normalizes_as_path_before_upstream_detection() {
        assert_eq!(
            normalize_path(&[2914.into(), 20473.into(), 139589.into(), 139589.into()]).as_slice(),
            &[2914.into(), 20473.into(), 139589.into()]
        );
        assert_eq!(
            normalize_path(&[1.into(), 2.into(), 3.into(), 4.into(), 5.into(), 5.into()])
                .as_slice(),
            &[2.into(), 3.into(), 4.into(), 5.into()]
        );
    }

    #[test]
    fn preserves_policy_input_error_context() {
        let trusted_error = DomesticPolicy::from_files(
            Path::new("/nonexistent/china-operator-ip-trusted-asns"),
            Path::new("/dev/null"),
        )
        .err()
        .unwrap();
        assert!(
            trusted_error
                .to_string()
                .starts_with("failed to load trusted CN transit ASNs:")
        );

        let country_error = DomesticPolicy::from_files(
            Path::new("/dev/null"),
            Path::new("/nonexistent/china-operator-ip-asn-countries"),
        )
        .err()
        .unwrap();
        assert!(country_error.to_string().starts_with(
            "failed to load ASN country data from /nonexistent/china-operator-ip-asn-countries:"
        ));
    }

    #[test]
    fn detects_trusted_transit_in_contiguous_cn_suffix() {
        let policy = DomesticPolicy {
            trusted_transit_asns: HashSet::from([Asn::from(4538), Asn::from(7497)]),
            asn_countries: HashMap::from([
                (Asn::from(24489), "CN".to_string()),
                (Asn::from(23911), "CN".to_string()),
                (Asn::from(4538), "CN".to_string()),
                (Asn::from(38345), "CN".to_string()),
                (Asn::from(7497), "CN".to_string()),
                (Asn::from(6939), "US".to_string()),
            ]),
        };

        assert!(policy.has_domestic_suffix(&[
            6939.into(),
            4538.into(),
            23911.into(),
            24489.into()
        ]));
        assert!(policy.has_domestic_suffix(&[6939.into(), 7497.into(), 38345.into()]));
        assert!(!policy.has_domestic_suffix(&[4538.into(), 6939.into(), 24489.into()]));
        assert!(!policy.has_domestic_suffix(&[4538.into(), 64500.into(), 24489.into()]));
    }

    #[test]
    fn parses_asn_country_lines() {
        assert_eq!(
            parse_country("AS4134 CHINANET-BACKBONE, CN"),
            Some((4134.into(), "CN"))
        );
        assert_eq!(
            parse_country("AS15169 GOOGLE, US"),
            Some((15169.into(), "US"))
        );
        assert_eq!(parse_country("invalid"), None);
    }

    #[test]
    fn detects_foreign_upstream_only_asns() {
        let targets = HashSet::from([1.into(), 2.into(), 3.into(), 4.into()]);
        let direct_upstreams = HashMap::from([
            (1.into(), BTreeSet::from([100.into(), 101.into()])),
            (2.into(), BTreeSet::from([100.into(), 102.into()])),
            (3.into(), BTreeSet::from([200.into()])),
            (4.into(), BTreeSet::new()),
        ]);
        let countries = HashMap::from([
            (100.into(), "US".to_string()),
            (101.into(), "JP".to_string()),
            (102.into(), "CN".to_string()),
        ]);

        assert_eq!(
            foreign_upstream_only(&targets, &direct_upstreams, &countries, "CN"),
            vec![Asn::from(1)]
        );
    }
}
