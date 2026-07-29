use std::error::Error;
use std::hash::{Hash, Hasher};
use std::path::Path;

crate::fixture_program! {
    pub struct OutputMultiWorker;
    relation edge(i32, i32);
    relation source(i32);
    relation reach(i32, i32, i32);
    relation closest(i32, i32, i32);

    reach(source_id, source_id, 0) :- source(source_id);
    reach(source_id, destination, minimum) :-
        reach(source_id, middle, distance),
        agg minimum = min(*distance + 1) in edge(middle, destination);
    closest(source_id, destination, distance) :- reach(source_id, destination, distance);
}

struct FnvHasher(u64);

impl Default for FnvHasher {
    fn default() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
}

impl Hasher for FnvHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }
}

fn worker_bucket(source: i32, destination: i32) -> usize {
    let mut hasher = FnvHasher::default();
    (source, destination).hash(&mut hasher);
    match hasher.finish() % 4 {
        0 => 0,
        2 => 1,
        1 => 2,
        3 => 3,
        _ => unreachable!(),
    }
}

pub fn run(fixture_dir: &Path, output_dir: &Path) -> Result<(), Box<dyn Error>> {
    let mut program = OutputMultiWorker {
        edge: crate::common::read(fixture_dir, "Edge.csv")?,
        source: crate::common::read(fixture_dir, "Source.csv")?,
        ..OutputMultiWorker::default()
    };
    program.run();
    program.reach.sort_by_key(|(source, destination, _)| {
        (worker_bucket(*source, *destination), *source, *destination)
    });
    program
        .closest
        .sort_by_key(|(_, destination, distance)| (*distance, *destination));
    program.closest.truncate(3);
    crate::common::write_rows(
        output_dir,
        "Reach.csv",
        program
            .reach
            .into_iter()
            .map(|(source, destination, distance)| format!("{source},{destination},{distance}")),
    )?;
    crate::common::write_rows(
        output_dir,
        "Closest.csv",
        program
            .closest
            .into_iter()
            .map(|(source, destination, distance)| format!("{source},{destination},{distance}")),
    )
}
