use std::collections::BTreeSet;
use std::fs;
use std::str::FromStr;

use breeze_icici::domain::{DerivativeExchange, Exchange, OptionRight};
use breeze_icici::instruments::{
    InstrumentKey, MasterFileKind, ScriptCode, ScriptDataKind, SecurityMaster,
};

use crate::support::{date, fixture_path, money, stock};

const MASTER_FILES: [&str; 7] = [
    "BSEScripMaster.txt",
    "CDNSEScripMaster.txt",
    "FOBSEScripMaster.txt",
    "FONSEScripMaster.txt",
    "MCXScripMaster.txt",
    "MFScripMaster.txt",
    "NSEScripMaster.txt",
];

fn load_master() -> SecurityMaster {
    let mut master = SecurityMaster::new();
    for filename in MASTER_FILES {
        let bytes = fs::read(fixture_path(&format!("security_master/{filename}"))).unwrap();
        master.ingest_file(filename, &bytes[..]).unwrap();
    }
    master
}

#[test]
fn every_observed_filename_selects_the_correct_schema() {
    let expected = [
        ("BSEScripMaster.txt", MasterFileKind::BseCash),
        ("CDNSEScripMaster.txt", MasterFileKind::NseCurrency),
        ("FOBSEScripMaster.txt", MasterFileKind::BseDerivatives),
        ("FONSEScripMaster.txt", MasterFileKind::NseDerivatives),
        ("MCXScripMaster.txt", MasterFileKind::Mcx),
        ("MFScripMaster.txt", MasterFileKind::MutualFunds),
        ("NSEScripMaster.txt", MasterFileKind::NseCash),
    ];

    for (filename, kind) in expected {
        assert_eq!(MasterFileKind::from_filename(filename), Some(kind));
    }
}

#[test]
fn all_schema_fixtures_parse_without_fixed_column_assumptions() {
    let master = load_master();
    let kinds: BTreeSet<_> = master.loaded_file_kinds().collect();
    assert_eq!(kinds.len(), MASTER_FILES.len());
    assert_eq!(master.diagnostics().malformed_rows(), 0);
}

#[test]
fn quoted_commas_and_headers_with_spaces_are_preserved() {
    let master = load_master();
    let nse = master
        .lookup(&InstrumentKey::equity(Exchange::Nse, stock("TESTCO")))
        .unwrap();
    assert_eq!(nse.company_name(), Some("TEST COMPANY, LIMITED"));
    assert_eq!(nse.token().as_str(), "1001");
}

#[test]
fn derivatives_lookup_uses_full_contract_identity_not_symbol_alone() {
    let master = load_master();
    let key = InstrumentKey::option(
        DerivativeExchange::Nfo,
        stock("NIFTY"),
        date("2025-02-27"),
        OptionRight::Call,
        money("24000"),
    );
    let instrument = master.lookup(&key).unwrap();
    assert_eq!(instrument.token().as_str(), "2001");

    let wrong_strike = InstrumentKey::option(
        DerivativeExchange::Nfo,
        stock("NIFTY"),
        date("2025-02-27"),
        OptionRight::Call,
        money("24100"),
    );
    assert!(master.lookup(&wrong_strike).is_none());
}

#[test]
fn script_code_parser_distinguishes_quotes_and_depth() {
    let quote = ScriptCode::from_str("4.1!1001").unwrap();
    assert_eq!(quote.exchange_qualifier(), 4);
    assert_eq!(quote.data_kind(), ScriptDataKind::Quotes);
    assert_eq!(quote.token().as_str(), "1001");
    assert_eq!(quote.to_string(), "4.1!1001");

    let depth = ScriptCode::from_str("4.2!1001").unwrap();
    assert_eq!(depth.data_kind(), ScriptDataKind::MarketDepth);

    for malformed in [
        "", "4!1001", "4.3!1001", "4.1", "x.1!1001", "4.1!", "4.1!1!2",
    ] {
        assert!(ScriptCode::from_str(malformed).is_err(), "{malformed:?}");
    }
}

#[test]
fn unknown_columns_are_additive_but_missing_identity_columns_fail_that_file() {
    let with_extra = b"\"Token\",\"ShortName\",\"Series\",\"CompanyName\",\"ExchangeCode\",\"FutureColumn\"\n\"7001\",\"EXTRA\",\"EQ\",\"EXTRA COMPANY\",\"EXTRA\",\"future-value\"\n";
    let mut master = SecurityMaster::new();
    master
        .ingest_file("NSEScripMaster.txt", &with_extra[..])
        .unwrap();
    assert!(
        master
            .lookup(&InstrumentKey::equity(Exchange::Nse, stock("EXTRA")))
            .is_some()
    );

    let missing_token = b"\"ShortName\",\"CompanyName\"\n\"BROKEN\",\"BROKEN COMPANY\"\n";
    assert!(
        master
            .ingest_file("NSEScripMaster.txt", &missing_token[..])
            .is_err()
    );
}

#[test]
fn duplicate_keys_are_deterministic_and_reported() {
    let first = b"\"Token\",\"ShortName\",\"CompanyName\",\"ExchangeCode\"\n\"8001\",\"DUP\",\"FIRST\",\"DUP\"\n";
    let second = b"\"Token\",\"ShortName\",\"CompanyName\",\"ExchangeCode\"\n\"8002\",\"DUP\",\"SECOND\",\"DUP\"\n";
    let mut master = SecurityMaster::new();
    master
        .ingest_file("NSEScripMaster.txt", &first[..])
        .unwrap();
    master
        .ingest_file("NSEScripMaster.txt", &second[..])
        .unwrap();

    let found = master
        .lookup(&InstrumentKey::equity(Exchange::Nse, stock("DUP")))
        .unwrap();
    assert_eq!(found.token().as_str(), "8002", "latest complete file wins");
    assert_eq!(master.diagnostics().duplicates(), 1);
}

#[test]
fn parser_is_reader_only_and_does_not_need_a_network_or_cache_path() {
    let bytes = fs::read(fixture_path("security_master/NSEScripMaster.txt")).unwrap();
    let master = SecurityMaster::parse_file("NSEScripMaster.txt", &bytes[..]).unwrap();
    assert_eq!(master.len(), 1);
}
