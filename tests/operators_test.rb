require "fileutils"
require "json"
require "open3"
require "tmpdir"

# Fixed name samples test matching rules, not current network ownership.
# Review these expectations when intentionally changing operators.yaml; never refresh from the network.
root = File.expand_path("..", __dir__)
Dir.mktmpdir("china-operator-ip-operators") do |dir|
  FileUtils.cp(["#{root}/justfile", "#{root}/operators.yaml"], dir)
  File.write("#{dir}/asnames.txt", <<~ASN)
    AS4134 CHINANET-BACKBONE - No.31,Jin-rong Street, CN
    AS4538 ERX-CERNET-BKB - China Education and Research Network Center, CN
    AS4837 CHINA169-Backbone - CHINA UNICOM China169 Backbone, CN
    AS4847 CNIX-AP - China Networks Inter-Exchange, CN
    AS7497 CSTNET-AS - Computer Network Information Center, CN
    AS18344 CNCGROUP-BACKBONE-NORTH - USED IN NORTH CHINANET BACKBONE, CN
    AS23910 CNGI-CERNET2-AS-AP - China Next Generation Internet CERNET2, CN
    AS23911 CNGI-BJIX-AS-AP - China Next Generation Internet Beijing IX, CN
    AS24311 CNGI-CMNETV6-AS-AP - China Mobile Communications Corporation IPv6 network, CN
    AS38035 CNGI-SHIX-AS-AP - CNGI Shanghai IPv6 Internet Exchange Center, CN
    AS38270 CNGI-CI-AP - AS number for CNGI core network which is composed of, CN
    AS59265 CT-CNGI - China telecom - China Next Generation Internet, CN
    AS4262 CERNET-ASN-BLOCK - California Education and Research Federation Network, US
    AS45102 ALIBABA-CN-NET - Alibaba (US) Technology Co., Ltd., CN
  ASN
  expected = {
    "cernet" => %w[4538 23910 23911],
    "chinanet" => %w[4134 4847 38035 38270 59265],
    "cmcc" => %w[24311],
    "unicom" => %w[4837 18344],
    "cstnet" => %w[7497],
  }
  expected.each do |operator, asns|
    output, status = Open3.capture2("just", "get_asn_candidates", operator, chdir: dir)
    abort("Failed to get #{operator} ASNs") unless status.success?
    actual = output.split.sort_by(&:to_i)
    abort("#{operator}: expected #{asns}, got #{actual}") unless actual == asns
  end
  output, status = Open3.capture2("just", "operator_asns", chdir: dir)
  abort("Failed to get operator boundaries") unless status.success?
  abort("Incorrect operator boundaries") unless output.split == expected.values.flatten.sort_by(&:to_i)

  # Exercise the generation recipe offline; Rust integration tests cover subtraction.
  File.write("#{dir}/rib-fixture.gz", "")
  %w[afrinic apnic arin lacnic ripencc].each do |registry|
    File.write("#{dir}/delegated-#{registry}-extended-latest",
      "#{registry}|CN|ipv4|192.0.2.0|256|20260101|allocated\n")
  end
  FileUtils.mkdir_p("#{dir}/target/release")
  classifier = "#{dir}/target/release/china-operator-ip"
  File.write(classifier, <<~RUBY)
    #!/usr/bin/env ruby
    require "json"
    File.write("classifier-args.json", JSON.generate(ARGV))
    puts "192.0.2.0/24"
  RUBY
  FileUtils.chmod(0755, classifier)
  %w[china chinanet].each do |operator|
    _out, err, status = Open3.capture3("just", "gen", operator, chdir: dir)
    abort("Failed to generate #{operator}: #{err}") unless status.success?
    args = JSON.parse(File.read("#{dir}/classifier-args.json"))
    index = args.index("--exclude-prefix-file")
    if operator == "china"
      abort("Missing prefix exclusions") unless index
      prefixes = File.readlines(File.join(dir, args[index + 1]), chomp: true)
      abort("Missing Singapore exclusion") unless prefixes.include?("47.236.0.0/16")
      abort("Excluded the entire Alibaba ASN") unless args.include?("45102")
    else
      abort("Unexpected exclusions for #{operator}") if index
    end
  end
end
puts "Operator ASN classification checks passed"
