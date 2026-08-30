use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::hash::Hash;
use std::io::Read;
use std::str::FromStr;

use chrono::NaiveDate;

use crate::domain::{DerivativeExchange, Exchange, Money, OptionRight, StockCode};
use crate::error::ValidationError;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum MasterFileKind {
    BseCash,
    NseCurrency,
    BseDerivatives,
    NseDerivatives,
    Mcx,
    MutualFunds,
    NseCash,
}

impl MasterFileKind {
    pub fn from_filename(filename: &str) -> Option<Self> {
        match filename.rsplit('/').next()? {
            "BSEScripMaster.txt" => Some(Self::BseCash),
            "CDNSEScripMaster.txt" => Some(Self::NseCurrency),
            "FOBSEScripMaster.txt" => Some(Self::BseDerivatives),
            "FONSEScripMaster.txt" => Some(Self::NseDerivatives),
            "MCXScripMaster.txt" => Some(Self::Mcx),
            "MFScripMaster.txt" => Some(Self::MutualFunds),
            "NSEScripMaster.txt" => Some(Self::NseCash),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum InstrumentKey {
    Equity {
        exchange: Exchange,
        stock_code: StockCode,
    },
    Future {
        exchange: DerivativeExchange,
        stock_code: StockCode,
        expiry: NaiveDate,
    },
    Option {
        exchange: DerivativeExchange,
        stock_code: StockCode,
        expiry: NaiveDate,
        right: OptionRight,
        strike: Money,
    },
}

impl InstrumentKey {
    pub fn equity(exchange: Exchange, stock_code: StockCode) -> Self {
        Self::Equity {
            exchange,
            stock_code,
        }
    }
    pub fn future(exchange: DerivativeExchange, stock_code: StockCode, expiry: NaiveDate) -> Self {
        Self::Future {
            exchange,
            stock_code,
            expiry,
        }
    }
    pub fn option(
        exchange: DerivativeExchange,
        stock_code: StockCode,
        expiry: NaiveDate,
        right: OptionRight,
        strike: Money,
    ) -> Self {
        Self::Option {
            exchange,
            stock_code,
            expiry,
            right,
            strike,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct InstrumentToken(String);
impl InstrumentToken {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug)]
pub struct InstrumentRecord {
    key: InstrumentKey,
    token: InstrumentToken,
    company_name: Option<String>,
}
impl InstrumentRecord {
    pub fn key(&self) -> &InstrumentKey {
        &self.key
    }
    pub fn token(&self) -> &InstrumentToken {
        &self.token
    }
    pub fn company_name(&self) -> Option<&str> {
        self.company_name.as_deref()
    }
}

#[derive(Clone, Debug, Default)]
pub struct InstrumentDiagnostics {
    malformed_rows: usize,
    duplicates: usize,
}
impl InstrumentDiagnostics {
    pub fn malformed_rows(&self) -> usize {
        self.malformed_rows
    }
    pub fn duplicates(&self) -> usize {
        self.duplicates
    }
}

#[derive(Clone, Debug, Default)]
pub struct SecurityMaster {
    records: HashMap<InstrumentKey, InstrumentRecord>,
    loaded: BTreeSet<MasterFileKind>,
    diagnostics: InstrumentDiagnostics,
}

impl SecurityMaster {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn parse_file(filename: &str, reader: impl Read) -> Result<Self, ValidationError> {
        let mut value = Self::new();
        value.ingest_file(filename, reader)?;
        Ok(value)
    }

    pub fn ingest_file(
        &mut self,
        filename: &str,
        reader: impl Read,
    ) -> Result<(), ValidationError> {
        let kind = MasterFileKind::from_filename(filename)
            .ok_or_else(|| ValidationError::new("unknown security-master filename"))?;
        let mut csv = csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .flexible(false)
            .from_reader(reader);
        let raw_headers = csv
            .headers()
            .map_err(|_| ValidationError::new("security-master header is invalid"))?
            .clone();
        let headers: Vec<String> = raw_headers.iter().map(normalize_header).collect();

        if kind != MasterFileKind::MutualFunds && !headers.iter().any(|value| value == "token") {
            return Err(ValidationError::new(
                "security-master token column is missing",
            ));
        }
        if matches!(kind, MasterFileKind::NseCash | MasterFileKind::BseCash)
            && !headers.iter().any(|value| value == "shortname")
        {
            return Err(ValidationError::new(
                "security-master short-name column is missing",
            ));
        }

        for row in csv.records() {
            let row = row.map_err(|_| {
                ValidationError::new("security-master row has an invalid CSV shape")
            })?;
            if row.iter().all(|value| value.trim().is_empty()) {
                continue;
            }
            match parse_record(kind, &headers, &row) {
                Ok(Some(record)) => {
                    if self.records.insert(record.key.clone(), record).is_some() {
                        self.diagnostics.duplicates += 1;
                    }
                }
                Ok(None) => {}
                Err(_) => self.diagnostics.malformed_rows += 1,
            }
        }
        self.loaded.insert(kind);
        Ok(())
    }

    pub fn lookup(&self, key: &InstrumentKey) -> Option<&InstrumentRecord> {
        self.records.get(key)
    }
    pub fn loaded_file_kinds(&self) -> impl Iterator<Item = MasterFileKind> + '_ {
        self.loaded.iter().copied()
    }
    pub fn diagnostics(&self) -> &InstrumentDiagnostics {
        &self.diagnostics
    }
    pub fn len(&self) -> usize {
        self.records.len()
    }
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

fn normalize_header(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace() && *ch != '_')
        .flat_map(char::to_lowercase)
        .collect()
}

fn field<'a>(headers: &[String], row: &'a csv::StringRecord, name: &str) -> Option<&'a str> {
    headers
        .iter()
        .position(|header| header == name)
        .and_then(|index| row.get(index))
        .map(str::trim)
}

fn required<'a>(
    headers: &[String],
    row: &'a csv::StringRecord,
    name: &str,
) -> Result<&'a str, ValidationError> {
    field(headers, row, name)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ValidationError::new(format!("security-master {name} is missing")))
}

fn parse_record(
    kind: MasterFileKind,
    headers: &[String],
    row: &csv::StringRecord,
) -> Result<Option<InstrumentRecord>, ValidationError> {
    if kind == MasterFileKind::MutualFunds {
        return Ok(None);
    }
    let token = InstrumentToken(required(headers, row, "token")?.to_owned());
    let code = StockCode::new(required(headers, row, "shortname")?)?;
    let company_name = field(headers, row, "companyname")
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let key = match kind {
        MasterFileKind::NseCash => InstrumentKey::equity(Exchange::Nse, code),
        MasterFileKind::BseCash => InstrumentKey::equity(Exchange::Bse, code),
        MasterFileKind::NseDerivatives
        | MasterFileKind::BseDerivatives
        | MasterFileKind::NseCurrency
        | MasterFileKind::Mcx => {
            let exchange = match kind {
                MasterFileKind::NseDerivatives => DerivativeExchange::Nfo,
                MasterFileKind::BseDerivatives => DerivativeExchange::Bfo,
                MasterFileKind::NseCurrency => DerivativeExchange::Ndx,
                MasterFileKind::Mcx => DerivativeExchange::Mcx,
                _ => unreachable!(),
            };
            let expiry = parse_date(required(headers, row, "expirydate")?)?;
            let option_type = field(headers, row, "optiontype").unwrap_or("");
            if let Some(right) = OptionRight::from_wire(option_type) {
                let strike = Money::from_str(required(headers, row, "strikeprice")?)?;
                InstrumentKey::option(exchange, code, expiry, right, strike)
            } else {
                InstrumentKey::future(exchange, code, expiry)
            }
        }
        MasterFileKind::MutualFunds => unreachable!(),
    };
    Ok(Some(InstrumentRecord {
        key,
        token,
        company_name,
    }))
}

fn parse_date(value: &str) -> Result<NaiveDate, ValidationError> {
    for format in ["%d-%b-%Y", "%Y-%m-%d"] {
        if let Ok(value) = NaiveDate::parse_from_str(value, format) {
            return Ok(value);
        }
    }
    Err(ValidationError::new(
        "security-master expiry date is invalid",
    ))
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ScriptDataKind {
    Quotes,
    MarketDepth,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ScriptCode {
    exchange_qualifier: u16,
    data_kind: ScriptDataKind,
    token: InstrumentToken,
}
impl ScriptCode {
    pub fn exchange_qualifier(&self) -> u16 {
        self.exchange_qualifier
    }
    pub fn data_kind(&self) -> ScriptDataKind {
        self.data_kind
    }
    pub fn token(&self) -> &InstrumentToken {
        &self.token
    }
}
impl FromStr for ScriptCode {
    type Err = ValidationError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (prefix, token) = value
            .split_once('!')
            .ok_or_else(|| ValidationError::new("invalid script code"))?;
        if token.is_empty() || token.contains('!') {
            return Err(ValidationError::new("invalid script token"));
        }
        let (exchange, kind) = prefix
            .split_once('.')
            .ok_or_else(|| ValidationError::new("invalid script qualifier"))?;
        let exchange_qualifier = exchange
            .parse()
            .map_err(|_| ValidationError::new("invalid script exchange qualifier"))?;
        let data_kind = match kind {
            "1" => ScriptDataKind::Quotes,
            "2" => ScriptDataKind::MarketDepth,
            _ => return Err(ValidationError::new("invalid script data kind")),
        };
        Ok(Self {
            exchange_qualifier,
            data_kind,
            token: InstrumentToken(token.to_owned()),
        })
    }
}
impl fmt::Display for ScriptCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}.{}!{}",
            self.exchange_qualifier,
            match self.data_kind {
                ScriptDataKind::Quotes => 1,
                ScriptDataKind::MarketDepth => 2,
            },
            self.token.0
        )
    }
}
