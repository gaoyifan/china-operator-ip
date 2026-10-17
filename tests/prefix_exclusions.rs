use bgpkit_parser::{
    encoder::MrtRibEncoder,
    models::{AsPath, BgpElem},
};
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

struct Fixture(PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn exclusions_override_bgp_and_fallback_without_excluding_the_origin() {
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "china-operator-ip-prefix-exclusions-{}",
        std::process::id()
    )));
    fs::create_dir(&fixture.0).unwrap();
    let mut encoder = MrtRibEncoder::new();
    // Synthetic routing data: both IPv4 ranges share an origin. IPv6 is fallback-only.
    for prefix in ["47.236.0.0/16", "47.237.0.0/16"] {
        encoder.process_elem(&BgpElem {
            timestamp: 1.0,
            peer_ip: "192.0.2.1".parse().unwrap(),
            peer_asn: 64497.into(),
            prefix: prefix.parse().unwrap(),
            as_path: Some(AsPath::from_sequence([64497, 64496])),
            ..Default::default()
        });
    }
    fs::write(fixture.0.join("rib.mrt"), encoder.export_bytes()).unwrap();
    fs::write(
        fixture.0.join("fallback.txt"),
        "47.236.0.0/15\n2001:db8::/32\n",
    )
    .unwrap();
    let run = |exclude: bool| -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_china-operator-ip"));
        command.current_dir(&fixture.0).args([
            "--mrt-file",
            "rib.mrt",
            "--origin-only",
            "--cache",
            "--fallback-prefix-file",
            "fallback.txt",
        ]);
        if exclude {
            command.args(["--exclude-prefix-file", "excluded.txt"]);
        }
        command.arg("64496").output().unwrap()
    };
    let lines = |output: Output| -> String {
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout)
            .unwrap()
            .replace("\r\n", "\n")
    };
    assert_eq!(lines(run(false)), "47.236.0.0/15\n2001:db8::/32\n");
    // Later runs must use the cache; they cannot reparse this removed input.
    fs::remove_file(fixture.0.join("rib.mrt")).unwrap();
    fs::write(
        fixture.0.join("excluded.txt"),
        "47.236.0.0/16\n47.236.106.0/24\n2001:db8:8000::/33\n192.0.2.0/24\n",
    )
    .unwrap();
    assert_eq!(lines(run(true)), "47.237.0.0/16\n2001:db8::/33\n");

    // Change only the exclusions: the cached classification must remain reusable.
    fs::write(fixture.0.join("excluded.txt"), "47.237.0.0/16\n").unwrap();
    assert_eq!(lines(run(true)), "47.236.0.0/16\n2001:db8::/32\n");
    fs::write(fixture.0.join("excluded.txt"), "").unwrap();
    assert_eq!(lines(run(true)), lines(run(false)));

    // Invalid or missing exclusions must fail rather than silently emit unfiltered data.
    fs::write(fixture.0.join("excluded.txt"), "not-a-prefix\n").unwrap();
    let invalid = run(true);
    assert!(!invalid.status.success());
    assert!(invalid.stdout.is_empty());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("failed to load excluded prefixes"));
    fs::remove_file(fixture.0.join("excluded.txt")).unwrap();
    assert!(!run(true).status.success());
}
