use std::error::Error;
use std::fmt::Display;
use std::fs;
use std::path::Path;
use std::str::FromStr;

pub type Row3<T> = (T, T, T);

pub trait RelationRow: Sized {
    fn parse(fields: &[String], filename: &str) -> Result<Self, Box<dyn Error>>;
}

macro_rules! relation_row {
    ($($type:ident : $index:tt),+ $(,)?) => {
        impl<$($type),+> RelationRow for ($($type,)+)
        where
            $($type: FromStr, <$type as FromStr>::Err: Display,)+
        {
            fn parse(fields: &[String], filename: &str) -> Result<Self, Box<dyn Error>> {
                let expected = 0 $(+ { let _ = stringify!($type); 1 })+;
                if fields.len() != expected {
                    return Err(format!(
                        "{filename}: expected {expected} columns, found {}",
                        fields.len(),
                    ).into());
                }
                Ok(($(
                    fields[$index].parse::<$type>().map_err(|error| {
                        format!(
                            "{filename}: cannot parse column {} value {:?}: {error}",
                            $index + 1,
                            fields[$index],
                        )
                    })?,
                )+))
            }
        }
    };
}

relation_row!(A: 0);
relation_row!(A: 0, B: 1);
relation_row!(A: 0, B: 1, C: 2);
relation_row!(A: 0, B: 1, C: 2, D: 3);
relation_row!(A: 0, B: 1, C: 2, D: 3, E: 4);
relation_row!(A: 0, B: 1, C: 2, D: 3, E: 4, F: 5);
relation_row!(A: 0, B: 1, C: 2, D: 3, E: 4, F: 5, G: 6);
relation_row!(A: 0, B: 1, C: 2, D: 3, E: 4, F: 5, G: 6, H: 7);

pub fn read<T: RelationRow>(fixture_dir: &Path, filename: &str) -> Result<Vec<T>, Box<dyn Error>> {
    read_delimited(fixture_dir, filename, ',')
}

pub fn read_delimited<T: RelationRow>(
    fixture_dir: &Path,
    filename: &str,
    delimiter: char,
) -> Result<Vec<T>, Box<dyn Error>> {
    fs::read_to_string(fixture_dir.join("data").join(filename))?
        .lines()
        .map(|line| {
            let fields = line.split(delimiter).map(str::to_owned).collect::<Vec<_>>();
            T::parse(&fields, filename)
        })
        .collect()
}

pub fn read_i32_2(fixture_dir: &Path, filename: &str) -> Result<Vec<(i32, i32)>, Box<dyn Error>> {
    read(fixture_dir, filename)
}

pub fn read_i32_3(fixture_dir: &Path, filename: &str) -> Result<Vec<Row3<i32>>, Box<dyn Error>> {
    read(fixture_dir, filename)
}

pub trait OutputRow {
    fn to_output_line(self) -> String;
}

pub trait OutputValue {
    fn to_output_value(&self) -> String;
}

macro_rules! output_value_display {
    ($($type:ty),+ $(,)?) => {
        $(impl OutputValue for $type {
            fn to_output_value(&self) -> String {
                self.to_string()
            }
        })+
    };
}

output_value_display!(
    i8,
    i16,
    i32,
    i64,
    isize,
    u8,
    u16,
    u32,
    u64,
    usize,
    f32,
    f64,
    bool,
    String,
    ordered_float::OrderedFloat<f32>,
    ordered_float::OrderedFloat<f64>,
);

macro_rules! output_value_tuple {
    ($($type:ident : $index:tt),+ $(,)?) => {
        impl<$($type: OutputValue),+> OutputValue for ($($type,)+) {
            fn to_output_value(&self) -> String {
                let values = [$(self.$index.to_output_value(),)+];
                if values.len() == 1 {
                    format!("({},)", values[0])
                } else {
                    format!("({})", values.join(", "))
                }
            }
        }
    };
}

output_value_tuple!(A: 0);
output_value_tuple!(A: 0, B: 1);
output_value_tuple!(A: 0, B: 1, C: 2);
output_value_tuple!(A: 0, B: 1, C: 2, D: 3);
output_value_tuple!(A: 0, B: 1, C: 2, D: 3, E: 4);
output_value_tuple!(A: 0, B: 1, C: 2, D: 3, E: 4, F: 5);

impl OutputRow for () {
    fn to_output_line(self) -> String {
        "True".to_owned()
    }
}

macro_rules! output_row {
    ($first:ident : $first_index:tt $(, $rest:ident : $rest_index:tt)* $(,)?) => {
        impl<$first, $($rest),*> OutputRow for ($first, $($rest,)*)
        where
            $first: OutputValue,
            $($rest: OutputValue,)*
        {
            fn to_output_line(self) -> String {
                let values = [
                    self.$first_index.to_output_value(),
                    $(self.$rest_index.to_output_value(),)*
                ];
                values.join("\t")
            }
        }
    };
}

output_row!(A: 0);
output_row!(A: 0, B: 1);
output_row!(A: 0, B: 1, C: 2);
output_row!(A: 0, B: 1, C: 2, D: 3);
output_row!(A: 0, B: 1, C: 2, D: 3, E: 4);
output_row!(A: 0, B: 1, C: 2, D: 3, E: 4, F: 5);
output_row!(A: 0, B: 1, C: 2, D: 3, E: 4, F: 5, G: 6);
output_row!(A: 0, B: 1, C: 2, D: 3, E: 4, F: 5, G: 6, H: 7);

pub fn write<T: OutputRow>(
    output_dir: &Path,
    filename: &str,
    rows: Vec<T>,
) -> Result<(), Box<dyn Error>> {
    write_rows(
        output_dir,
        filename,
        rows.into_iter().map(OutputRow::to_output_line),
    )
}

pub fn write_i32_2(
    output_dir: &Path,
    filename: &str,
    rows: Vec<(i32, i32)>,
) -> Result<(), Box<dyn Error>> {
    write(output_dir, filename, rows)
}

pub fn write_rows(
    output_dir: &Path,
    filename: &str,
    rows: impl IntoIterator<Item = impl Display>,
) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(output_dir)?;
    let mut output = rows
        .into_iter()
        .map(|row| row.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    if !output.is_empty() {
        output.push('\n');
    }
    fs::write(output_dir.join(filename), output)?;
    Ok(())
}
