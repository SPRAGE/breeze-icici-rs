use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

const EXPECTED_REST_OPERATIONS: usize = 27;
const EXPECTED_STREAM_FAMILIES: usize = 5;

#[derive(Debug, Deserialize)]
struct Manifest {
    schema_version: u32,
    source: Source,
    documented_limits: DocumentedLimits,
    required_markdown: Vec<String>,
    example_targets: Vec<ExampleTarget>,
    rest_operations: Vec<RestOperation>,
    streams: Vec<StreamFamily>,
}

#[derive(Debug, Deserialize)]
struct ExampleTarget {
    name: String,
    path: String,
    required_features: Vec<String>,
    network_mode: String,
}

#[derive(Debug, Deserialize)]
struct Source {
    documentation_url: String,
    retrieved_at: String,
    sha256: String,
    official_python_sdk_url: String,
    official_python_sdk_version: String,
    official_python_sdk_commit: String,
}

#[derive(Debug, Deserialize)]
struct DocumentedLimits {
    rest_calls_per_minute: u32,
    rest_calls_per_day: u32,
    combined_order_mutations_per_second: u32,
    timestamp_skew_seconds: u32,
    stream_scripts: u32,
}

#[derive(Debug, Deserialize)]
struct RestOperation {
    id: String,
    section: String,
    method: String,
    base: String,
    path: String,
    auth: String,
    response_fixture: String,
}

#[derive(Debug, Deserialize)]
struct StreamFamily {
    id: String,
    section: String,
    url: String,
    socketio_path: String,
    event: String,
    fixture: String,
}

#[derive(Debug, Deserialize)]
struct WireContract {
    method: String,
    base: String,
    path: String,
    auth: String,
    body: String,
    #[serde(default)]
    query: Vec<(String, String)>,
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fixture_path(name: &str) -> PathBuf {
    repository_root().join("tests/fixtures").join(name)
}

fn read_json<T: for<'de> Deserialize<'de>>(name: &str) -> T {
    let path = fixture_path(name);
    let bytes = fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|error| panic!("parse {} as JSON: {error}", path.display()))
}

fn assert_lower_hex(value: &str, bytes: usize, label: &str) {
    assert_eq!(value.len(), bytes * 2, "{label} must contain {bytes} bytes");
    assert!(
        value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "{label} must be lowercase hexadecimal"
    );
}

#[test]
fn source_revision_and_documented_limits_are_pinned() {
    let manifest: Manifest = read_json("manifest.json");

    assert_eq!(manifest.schema_version, 2);
    assert_eq!(
        manifest.source.documentation_url,
        "https://api.icicidirect.com/breezeapi/documents/index.html"
    );
    assert!(manifest.source.retrieved_at.starts_with("2026-08-29T"));
    assert_lower_hex(&manifest.source.sha256, 32, "documentation SHA-256");
    assert_eq!(
        manifest.source.official_python_sdk_url,
        "https://github.com/Idirect-Tech/Breeze-Python-SDK"
    );
    assert_eq!(manifest.source.official_python_sdk_version, "1.0.68");
    assert_lower_hex(
        &manifest.source.official_python_sdk_commit,
        20,
        "official Python SDK commit",
    );

    assert_eq!(manifest.documented_limits.rest_calls_per_minute, 100);
    assert_eq!(manifest.documented_limits.rest_calls_per_day, 5_000);
    assert_eq!(
        manifest
            .documented_limits
            .combined_order_mutations_per_second,
        10
    );
    assert_eq!(manifest.documented_limits.timestamp_skew_seconds, 60);
    assert_eq!(manifest.documented_limits.stream_scripts, 2_000);
}

#[test]
fn declared_examples_exist_compile_as_cargo_targets_and_never_dispatch_mutations() {
    let root = repository_root();
    let manifest: Manifest = read_json("manifest.json");
    let cargo = fs::read_to_string(root.join("Cargo.toml")).expect("read Cargo.toml");

    assert_eq!(manifest.example_targets.len(), 8);
    let mut names = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for example in &manifest.example_targets {
        assert!(
            names.insert(example.name.as_str()),
            "duplicate example name"
        );
        assert!(
            paths.insert(example.path.as_str()),
            "duplicate example path"
        );
        assert!(
            matches!(
                example.network_mode.as_str(),
                "read_only" | "read_only_stream" | "offline"
            ),
            "unknown example network mode {}",
            example.network_mode
        );

        let path = root.join(&example.path);
        assert!(path.is_file(), "example {} is missing", path.display());
        let target = cargo
            .split("[[example]]")
            .find(|block| block.contains(&format!("name = {:?}", example.name)))
            .unwrap_or_else(|| panic!("{} is not declared as a Cargo example", example.name));
        assert!(target.contains(&format!("path = {:?}", example.path)));
        for feature in &example.required_features {
            assert!(
                target.contains(&format!("required-features = [{feature:?}]")),
                "{} does not require feature {}",
                example.name,
                feature
            );
        }
    }

    let offline = manifest
        .example_targets
        .iter()
        .find(|example| example.name == "mutation_requests")
        .expect("offline mutation-construction example");
    assert_eq!(offline.network_mode, "offline");
    let source = fs::read_to_string(root.join(&offline.path)).expect("read offline example");
    assert!(!source.contains("BreezeClient"));
    assert!(!source.contains("authenticated_client"));
    assert!(!source.contains("reqwest"));
    assert!(!source.contains("std::net"));
    assert!(!source.contains("tokio"));

    let mut example_files = Vec::new();
    collect_files(&root.join("examples"), &mut example_files);
    for path in example_files
        .into_iter()
        .filter(|path| path.extension().is_some_and(|value| value == "rs"))
    {
        let source = fs::read_to_string(&path).expect("read example source");
        for dispatch in [".trading()", ".set_funds(", ".execute("] {
            assert!(
                !source.contains(dispatch),
                "{} contains live mutation dispatch {dispatch}",
                path.display()
            );
        }
    }
}

#[test]
fn every_documented_rest_operation_has_one_wire_contract_and_response_fixture() {
    let manifest: Manifest = read_json("manifest.json");
    let wire: BTreeMap<String, WireContract> = read_json("wire_contracts.json");
    let responses: BTreeMap<String, Value> = read_json("rest_success.json");

    assert_eq!(manifest.rest_operations.len(), EXPECTED_REST_OPERATIONS);
    assert_eq!(wire.len(), EXPECTED_REST_OPERATIONS);
    assert_eq!(responses.len(), EXPECTED_REST_OPERATIONS);

    let operation_ids: BTreeSet<_> = manifest
        .rest_operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    assert_eq!(
        operation_ids.len(),
        EXPECTED_REST_OPERATIONS,
        "operation IDs must be unique"
    );
    assert_eq!(
        operation_ids,
        wire.keys().map(String::as_str).collect(),
        "wire contracts must match the endpoint inventory exactly"
    );

    for operation in &manifest.rest_operations {
        assert!(
            !operation.section.trim().is_empty(),
            "{} has no source section",
            operation.id
        );
        assert!(matches!(
            operation.method.as_str(),
            "GET" | "POST" | "PUT" | "DELETE"
        ));
        assert!(matches!(operation.base.as_str(), "rest_v1" | "rest_v2"));
        assert!(operation.path.starts_with('/'));
        assert!(matches!(
            operation.auth.as_str(),
            "session_exchange" | "signed_v1" | "session_v2"
        ));

        let contract = &wire[&operation.id];
        assert_eq!(
            contract.method, operation.method,
            "{} method drift",
            operation.id
        );
        assert_eq!(contract.base, operation.base, "{} base drift", operation.id);
        assert_eq!(contract.path, operation.path, "{} path drift", operation.id);
        assert_eq!(contract.auth, operation.auth, "{} auth drift", operation.id);

        if contract.base == "rest_v2" {
            assert!(
                contract.body.is_empty(),
                "v2 contract must not have a JSON body"
            );
            assert!(
                !contract.query.is_empty(),
                "v2 contract must have query pairs"
            );
            let query_keys: BTreeSet<_> =
                contract.query.iter().map(|(key, _)| key.as_str()).collect();
            assert!(query_keys.contains("exch_code"));
            assert!(!query_keys.contains("exchange_code"));
        } else {
            let body: Value = serde_json::from_str(&contract.body)
                .unwrap_or_else(|error| panic!("{} body is invalid JSON: {error}", operation.id));
            assert!(
                body.is_object(),
                "{} body must be a JSON object",
                operation.id
            );
            assert!(
                !contract.body.contains(": ") && !contract.body.contains(", "),
                "{} body must be compact JSON",
                operation.id
            );
        }

        let response = &responses[&operation.response_fixture];
        let response_object = response
            .as_object()
            .unwrap_or_else(|| panic!("{} response must be an object", operation.id));
        assert!(response_object.contains_key("Success"));
        assert_eq!(response_object.get("Status"), Some(&Value::from(200)));
        assert_eq!(response_object.get("Error"), Some(&Value::Null));
    }

    assert_eq!(wire["market.option_chain"].path, "/optionchain");
    assert_eq!(wire["account.get_funds"].body, "{}");
    assert_eq!(wire["account.demat_holdings"].body, "{}");
    assert_eq!(wire["portfolio.positions"].body, "{}");
}

#[test]
fn all_stream_families_have_https_socketio_contracts_and_fixtures() {
    let manifest: Manifest = read_json("manifest.json");
    let frames: BTreeMap<String, Value> = read_json("stream_frames.json");

    assert_eq!(manifest.streams.len(), EXPECTED_STREAM_FAMILIES);
    let ids: BTreeSet<_> = manifest
        .streams
        .iter()
        .map(|stream| stream.id.as_str())
        .collect();
    assert_eq!(
        ids.len(),
        EXPECTED_STREAM_FAMILIES,
        "stream IDs must be unique"
    );

    let expected_ids = BTreeSet::from([
        "stream.orders",
        "stream.market_data",
        "stream.one_click_fno",
        "stream.one_click_equity",
        "stream.candles",
    ]);
    assert_eq!(ids, expected_ids);

    for stream in &manifest.streams {
        assert!(!stream.section.trim().is_empty());
        assert!(stream.url.starts_with("https://"));
        assert!(stream.socketio_path.starts_with('/'));
        assert!(!stream.event.is_empty());
        assert!(
            frames.contains_key(&stream.fixture),
            "{} references missing stream fixture {}",
            stream.id,
            stream.fixture
        );
    }

    assert_eq!(manifest.streams[0].event, "order");
    assert_eq!(manifest.streams[1].event, "stock");
    assert_eq!(manifest.streams[4].socketio_path, "/ohlcvstream");
}

#[test]
fn error_and_stream_corpora_cover_required_edge_shapes() {
    let errors: BTreeMap<String, Value> = read_json("errors.json");
    let frames: BTreeMap<String, Value> = read_json("stream_frames.json");

    for key in [
        "application_error_with_http_200",
        "bad_request",
        "unauthorized",
        "forbidden",
        "not_found",
        "request_timeout",
        "rate_limited",
        "server_error",
        "success_null",
        "empty_success_list",
        "unknown_status",
    ] {
        assert!(errors.contains_key(key), "missing error fixture {key}");
    }

    for key in [
        "stream_credentials",
        "quote_nse",
        "quote_nfo",
        "market_depth_bse",
        "market_depth_nse",
        "commodity",
        "order_cash",
        "order_derivative",
        "one_click_fno",
        "one_click_equity",
        "candle_equity",
        "candle_option",
        "candle_future",
        "unknown",
    ] {
        assert!(frames.contains_key(key), "missing stream fixture {key}");
    }
}

#[test]
fn every_observed_security_master_schema_has_a_small_fixture() {
    let directory = fixture_path("security_master");
    let expected: BTreeSet<String> = [
        "BSEScripMaster.txt",
        "CDNSEScripMaster.txt",
        "FOBSEScripMaster.txt",
        "FONSEScripMaster.txt",
        "MCXScripMaster.txt",
        "MFScripMaster.txt",
        "NSEScripMaster.txt",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let actual: BTreeSet<_> = fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
        .map(|entry| {
            entry
                .expect("security-master directory entry")
                .file_name()
                .into_string()
                .expect("UTF-8 fixture filename")
        })
        .collect();
    assert_eq!(actual, expected);

    for filename in actual {
        let text =
            fs::read_to_string(directory.join(&filename)).expect("read security-master fixture");
        let non_empty_lines: Vec<_> = text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .collect();
        assert_eq!(
            non_empty_lines.len(),
            2,
            "{filename} must contain a header and one row"
        );
        let header = non_empty_lines[0];
        assert!(
            header.contains("Token") || header.contains("CompanyCode"),
            "{filename} has no identifying token/company column"
        );
    }
}

#[test]
fn all_required_markdown_exists_and_declares_the_preview_release_boundary() {
    let root = repository_root();
    let manifest: Manifest = read_json("manifest.json");

    for relative in &manifest.required_markdown {
        let path = root.join(relative);
        assert!(
            path.is_file(),
            "required document {} is missing",
            path.display()
        );
        assert!(
            fs::read_to_string(&path)
                .expect("read required document")
                .trim()
                .len()
                > 100,
            "required document {} is unexpectedly empty",
            path.display()
        );
    }

    let readme = fs::read_to_string(root.join("README.md")).expect("read README");
    assert!(readme.contains("Version `0.0.1` is a source-only preview"));
    assert!(readme.contains("known limitations"));
    assert!(readme.contains("This codebase is AI-generated"));
    assert!(readme.contains("Distribution through crates.io is for evaluation"));
    assert!(readme.contains("not production-ready"));
    let plan = fs::read_to_string(root.join("docs/IMPLEMENTATION_PLAN.md")).expect("read plan");
    assert!(plan.contains("Milestone 8: stable and production qualification — intentionally open"));
    let limitations =
        fs::read_to_string(root.join("docs/KNOWN_LIMITATIONS.md")).expect("read known limitations");
    assert!(limitations.contains("before production use or a stable release"));
    assert!(root.join("src/lib.rs").is_file());
}

#[test]
fn github_release_automation_keeps_registry_credentials_narrow() {
    let root = repository_root();
    let workflow_root = root.join(".github/workflows");

    // Published crate archives intentionally omit repository automation.
    if !workflow_root.is_dir() {
        return;
    }

    let ci = fs::read_to_string(workflow_root.join("ci.yml")).expect("read CI workflow");
    let publish =
        fs::read_to_string(workflow_root.join("publish.yml")).expect("read publish workflow");

    assert!(ci.contains("pull_request:"));
    assert!(ci.contains("cargo +stable test --locked --all-features"));
    assert!(ci.contains("cargo +1.85.0 check --locked --lib --all-features"));
    assert!(ci.contains("actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1"));
    assert!(ci.contains("permissions:\n  contents: read"));
    assert!(ci.contains("persist-credentials: false"));
    assert!(!ci.contains("CARGO_REGISTRY_TOKEN"));
    assert!(!ci.contains("pull_request_target"));

    assert!(publish.contains("workflow_dispatch:"));
    assert!(publish.contains("name: crates-io"));
    assert!(publish.contains(
        "if: github.repository == 'SPRAGE/breeze-icici-rs' && github.ref == 'refs/heads/main'"
    ));
    assert!(publish.contains("actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1"));
    assert!(publish.contains("permissions:\n  contents: read"));
    assert!(publish.contains("ref: refs/tags/${{ steps.release.outputs.tag }}"));
    assert!(publish.contains("cargo +1.85.0 publish --dry-run --locked --registry crates-io"));
    assert!(publish.contains("cargo +1.85.0 publish --locked --no-verify --registry crates-io"));
    assert!(publish.contains("secrets.CARGO_REGISTRY_TOKEN"));
    assert!(publish.contains("actual_checksum"));
    assert!(publish.contains("persist-credentials: false"));
    assert!(!publish.contains("pull_request_target"));

    let (before_publish, _) = publish
        .split_once("- name: Publish to crates.io")
        .expect("publish step must be explicit");
    assert!(!before_publish.contains("CARGO_REGISTRY_TOKEN"));
}

#[test]
fn fixtures_do_not_contain_values_copied_from_official_credential_examples() {
    let fixture_root = fixture_path("");
    let forbidden = [
        "Your Secret_key goes here",
        "Your App_Key goes here",
        "SESSION_TOKEN_FROM_CUSTOMER_DETAILS_API",
        "QUYyOTUzMTM6NjY5ODc5NzY=",
        "AG570549",
        "8510232533",
        "8g791^N029R47I831B8153",
    ];

    let mut paths = Vec::new();
    collect_files(&fixture_root, &mut paths);
    for path in paths {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        for value in forbidden {
            assert!(
                !text.contains(value),
                "{} contains credential/example material {value:?}",
                path.display()
            );
        }
    }
}

fn collect_files(directory: &Path, output: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
    {
        let path = entry.expect("fixture directory entry").path();
        if path.is_dir() {
            collect_files(&path, output);
        } else {
            output.push(path);
        }
    }
}
