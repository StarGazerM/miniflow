//! Shared data loading, string interning, timing, and size reporting for the
//! pinned `flowlog-bench` programs.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Instant;

use dashmap::DashMap;
use serde::{Deserialize, Serialize};

/// Interned string key used by the string-heavy benchmark programs.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct IStr(u32);

struct Interner {
    map: DashMap<&'static str, u32, ahash::RandomState>,
    strings: boxcar::Vec<&'static str>,
}

fn interner() -> &'static Interner {
    static INTERNER: OnceLock<Interner> = OnceLock::new();
    INTERNER.get_or_init(|| Interner {
        map: DashMap::with_hasher(ahash::RandomState::new()),
        strings: boxcar::Vec::new(),
    })
}

/// Intern a string for use in a relation.
///
/// # Panics
///
/// Panics if more than `u32::MAX` distinct strings are interned.
#[must_use]
pub fn sym(value: &str) -> IStr {
    let interner = interner();
    if let Some(key) = interner.map.get(value) {
        return IStr(*key);
    }
    let owned: &'static str = Box::leak(value.to_owned().into_boxed_str());
    match interner.map.entry(owned) {
        dashmap::mapref::entry::Entry::Occupied(entry) => IStr(*entry.get()),
        dashmap::mapref::entry::Entry::Vacant(entry) => {
            let key = u32::try_from(interner.strings.push(owned)).expect("interner key overflow");
            entry.insert(key);
            IStr(key)
        }
    }
}

/// Resolve an interned key.
#[must_use]
pub fn res(key: IStr) -> &'static str {
    interner().strings[key.0 as usize]
}

impl std::fmt::Display for IStr {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(res(*self))
    }
}

/// Parse one delimited field into a relation column.
pub trait FromField: Sized {
    fn from_field(field: &str) -> Self;
}

macro_rules! impl_from_field_int {
    ($($ty:ty),* $(,)?) => {$(
        impl FromField for $ty {
            fn from_field(field: &str) -> Self {
                field.parse().unwrap_or_else(|error| {
                    panic!("bad {} field {field:?}: {error}", stringify!($ty))
                })
            }
        }
    )*};
}

impl_from_field_int!(i8, i16, i32, i64, u8, u16, u32, u64, usize);

impl FromField for IStr {
    fn from_field(field: &str) -> Self {
        sym(field)
    }
}

impl FromField for String {
    fn from_field(field: &str) -> Self {
        field.to_owned()
    }
}

/// Parse one delimited record into a relation tuple.
pub trait FromRecord: Sized {
    fn from_line(line: &str, delimiter: char) -> Self;
}

macro_rules! impl_from_record {
    ($($column:ident),+ $(,)?) => {
        impl<$($column: FromField),+> FromRecord for ($($column,)+) {
            fn from_line(line: &str, delimiter: char) -> Self {
                let mut fields = line.split(delimiter);
                let row = ($($column::from_field(
                    fields
                        .next()
                        .unwrap_or_else(|| panic!("missing field in line {line:?}")),
                ),)+);
                assert!(
                    fields.next().is_none(),
                    "extra fields in line {line:?}"
                );
                row
            }
        }
    };
}

impl_from_record!(A);
impl_from_record!(A, B);
impl_from_record!(A, B, C);
impl_from_record!(A, B, C, D);
impl_from_record!(A, B, C, D, E);
impl_from_record!(A, B, C, D, E, F);
impl_from_record!(A, B, C, D, E, F, G);
impl_from_record!(A, B, C, D, E, F, G, H);
impl_from_record!(A, B, C, D, E, F, G, H, I);
impl_from_record!(A, B, C, D, E, F, G, H, I, J);

/// Parse one RFC 4180 record into a relation tuple.
pub trait FromCsvRecord: Sized {
    fn from_csv_record(record: &csv::StringRecord) -> Self;
}

macro_rules! impl_from_csv_record {
    ($($column:ident),+ $(,)?) => {
        impl<$($column: FromField),+> FromCsvRecord for ($($column,)+) {
            fn from_csv_record(record: &csv::StringRecord) -> Self {
                let mut fields = record.iter();
                let row = ($($column::from_field(
                    fields
                        .next()
                        .unwrap_or_else(|| panic!("missing field in record {record:?}"))
                        .trim_end(),
                ),)+);
                assert!(
                    fields.next().is_none(),
                    "extra fields in record {record:?}"
                );
                row
            }
        }
    };
}

impl_from_csv_record!(A);
impl_from_csv_record!(A, B);
impl_from_csv_record!(A, B, C);
impl_from_csv_record!(A, B, C, D);
impl_from_csv_record!(A, B, C, D, E);
impl_from_csv_record!(A, B, C, D, E, F);
impl_from_csv_record!(A, B, C, D, E, F, G);
impl_from_csv_record!(A, B, C, D, E, F, G, H);
impl_from_csv_record!(A, B, C, D, E, F, G, H, I);
impl_from_csv_record!(A, B, C, D, E, F, G, H, I, J);

/// Load one relation file. Missing files denote empty relations, matching the
/// benchmark harness contract.
///
/// # Panics
///
/// Panics when an existing file cannot be read or a row does not match the
/// inferred relation schema.
#[must_use]
pub fn load_rel<C, T>(directory: &Path, file: &str, delimiter: char) -> C
where
    T: FromRecord + Send,
    C: FromIterator<T> + Default,
{
    use rayon::prelude::*;

    let path = directory.join(file);
    let parsed: Vec<T> = {
        let data = match std::fs::read_to_string(&path) {
            Ok(data) => data,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                eprintln!("warning: {} not found, relation left empty", path.display());
                return C::default();
            }
            Err(error) => panic!("cannot read {}: {error}", path.display()),
        };
        data.par_lines()
            .filter(|line| !line.is_empty())
            .map(|line| T::from_line(line, delimiter))
            .collect()
    };
    parsed.into_iter().collect()
}

/// Load a delimited relation through the CSV parser, optionally skipping its
/// header. This is used by the LDBC corpus, whose text columns may contain CSV
/// quoting and whose files all carry headers.
///
/// # Panics
///
/// Panics when an existing file cannot be read or parsed, or a record does not
/// match the inferred relation schema.
#[must_use]
pub fn load_rel_csv<C, T>(directory: &Path, file: &str, delimiter: u8, header: bool) -> C
where
    T: FromCsvRecord,
    C: FromIterator<T> + Default,
{
    let path = directory.join(file);
    let file = match std::fs::File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("warning: {} not found, relation left empty", path.display());
            return C::default();
        }
        Err(error) => panic!("cannot open {}: {error}", path.display()),
    };
    csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(header)
        .from_reader(file)
        .records()
        .map(|record| {
            let record =
                record.unwrap_or_else(|error| panic!("cannot parse {}: {error}", path.display()));
            T::from_csv_record(&record)
        })
        .collect()
}

/// Convert a relation row into delimited output fields.
pub trait ToRecord {
    fn to_record(&self) -> Vec<String>;
}

macro_rules! impl_to_record {
    ($($index:tt => $column:ident),+ $(,)?) => {
        impl<$($column: std::fmt::Display),+> ToRecord for ($($column,)+) {
            fn to_record(&self) -> Vec<String> {
                vec![$(self.$index.to_string()),+]
            }
        }
    };
}

impl_to_record!(0 => A);
impl_to_record!(0 => A, 1 => B);
impl_to_record!(0 => A, 1 => B, 2 => C);
impl_to_record!(0 => A, 1 => B, 2 => C, 3 => D);
impl_to_record!(0 => A, 1 => B, 2 => C, 3 => D, 4 => E);
impl_to_record!(0 => A, 1 => B, 2 => C, 3 => D, 4 => E, 5 => F);
impl_to_record!(0 => A, 1 => B, 2 => C, 3 => D, 4 => E, 5 => F, 6 => G);
impl_to_record!(0 => A, 1 => B, 2 => C, 3 => D, 4 => E, 5 => F, 6 => G, 7 => H);
impl_to_record!(0 => A, 1 => B, 2 => C, 3 => D, 4 => E, 5 => F, 6 => G, 7 => H, 8 => I);
impl_to_record!(
    0 => A,
    1 => B,
    2 => C,
    3 => D,
    4 => E,
    5 => F,
    6 => G,
    7 => H,
    8 => I,
    9 => J,
);

/// Write one relation using the same delimiter-only output contract as
/// `FlowLog`'s `.output` directive.
///
/// # Panics
///
/// Panics when the output directory or relation file cannot be written.
pub fn write_rel<T: ToRecord>(directory: &Path, file: &str, delimiter: u8, rows: &[T]) {
    std::fs::create_dir_all(directory)
        .unwrap_or_else(|error| panic!("cannot create {}: {error}", directory.display()));
    let path = directory.join(file);
    let mut writer = csv::WriterBuilder::new()
        .delimiter(delimiter)
        .has_headers(false)
        .from_path(&path)
        .unwrap_or_else(|error| panic!("cannot create {}: {error}", path.display()));
    for row in rows {
        writer
            .write_record(row.to_record())
            .unwrap_or_else(|error| panic!("cannot write {}: {error}", path.display()));
    }
    writer
        .flush()
        .unwrap_or_else(|error| panic!("cannot flush {}: {error}", path.display()));
}

/// Initialize the benchmark host and return the fact directory.
///
/// # Panics
///
/// Panics when `WORKERS` is not a positive integer, the global Rayon pool was
/// already initialized, or the fact-directory argument is missing.
#[must_use]
pub fn bench_init() -> PathBuf {
    if let Ok(workers) = std::env::var("WORKERS") {
        let workers: usize = workers.parse().expect("WORKERS must be an integer");
        miniflow::runtime::set_worker_count(workers);
        rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build_global()
            .expect("rayon pool already initialized");
    }
    PathBuf::from(
        std::env::args()
            .nth(1)
            .expect("usage: <program> <fact-dir>"),
    )
}

static LOAD_SECONDS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Measure the input-loading phase.
pub fn timed_load(load: impl FnOnce()) {
    let start = Instant::now();
    load();
    let elapsed = start.elapsed().as_secs_f64();
    LOAD_SECONDS.store(elapsed.to_bits(), std::sync::atomic::Ordering::Relaxed);
    println!("Data loaded for all inputs: {elapsed:.6}s");
}

/// Measure evaluation and emit FlowLog-compatible timing output.
pub fn timed_run(run: impl FnOnce()) {
    let start = Instant::now();
    run();
    let load = f64::from_bits(LOAD_SECONDS.load(std::sync::atomic::Ordering::Relaxed));
    println!(
        "Dataflow executed in {:.6}s",
        start.elapsed().as_secs_f64() + load
    );
}

/// Emit a FlowLog-compatible relation-size line.
pub fn printsize(relation: &str, size: usize) {
    println!("{}\t{size}", relation.to_lowercase());
}
