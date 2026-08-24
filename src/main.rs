mod asn;
mod cache;
mod classifier;
mod ip;

use asn::{Asn, DomesticPolicy, foreign_upstream_only, load_countries};
use cache::CacheKey;
use clap::{ArgAction, Parser};
use classifier::ClassifierConfig;
use ipnet::IpNet;
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(name = "china-operator-ip", version)]
struct Opts {
    #[arg(short, long = "mrt-file", value_name = "MRT", action = ArgAction::Append)]
    mrt_files: Vec<PathBuf>,

    #[arg(value_name = "ASN", num_args = 1..)]
    asns: Vec<Asn>,

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
    let opts = Opts::parse();
    let targets: HashSet<Asn> = opts.asns.iter().copied().collect();
    let domestic_policy = opts.trusted_cn_transit_file.as_deref().map(|trusted_path| {
        let country_path = opts
            .asn_country_file
            .as_deref()
            .expect("--asn-country-file is required with --trusted-cn-transit-file");
        DomesticPolicy::from_files(trusted_path, country_path).unwrap_or_else(|err| panic!("{err}"))
    });
    let config = ClassifierConfig {
        ignore_private_asn: opts.ignore_private_asn,
        origin_only: opts.origin_only,
    };
    let cache_key = CacheKey::new(
        opts.ignore_private_asn,
        opts.origin_only,
        domestic_policy.as_ref().map(DomesticPolicy::fingerprint),
    );

    let classification = if opts.cache {
        let path = cache_key.path(&opts.mrt_files);
        cache::load(&path, cache_key).unwrap_or_else(|| {
            cache::save(
                &path,
                cache_key,
                classifier::build(&opts.mrt_files, config, domestic_policy.as_ref()),
            )
        })
    } else {
        classifier::build(&opts.mrt_files, config, domestic_policy.as_ref())
    };

    let foreign_upstream_only_asns = match opts.exclude_foreign_upstream_only.as_deref() {
        Some(country) => {
            let country_path = opts
                .asn_country_file
                .as_deref()
                .expect("--asn-country-file is required with --exclude-foreign-upstream-only");
            let countries = load_countries(country_path).unwrap_or_else(|err| {
                panic!(
                    "failed to load ASN country data from {}: {err}",
                    country_path.display()
                )
            });
            foreign_upstream_only(
                &targets,
                classification.direct_upstreams(),
                &countries,
                country,
            )
        }
        None => {
            assert!(
                !opts.debug_print_foreign_upstream_only_asns,
                "--debug-print-foreign-upstream-only-asns requires --exclude-foreign-upstream-only"
            );
            Vec::new()
        }
    };

    if opts.debug_print_foreign_upstream_only_asns {
        for asn in foreign_upstream_only_asns {
            println!("{asn}");
        }
        return;
    }

    if opts.debug_print_seen_origin_asns {
        for asn in classification.seen(&targets) {
            println!("{asn}");
        }
        return;
    }

    let fallback_prefixes = opts
        .fallback_prefix_file
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
    let excluded: HashSet<Asn> = foreign_upstream_only_asns.into_iter().collect();
    for line in classification
        .result(&targets, &excluded, &fallback_prefixes)
        .lines()
    {
        println!("{line}");
    }
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
