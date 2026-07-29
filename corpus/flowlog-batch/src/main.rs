use std::env;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = env::args().skip(1);
    let fixture = arguments.next().ok_or("missing fixture name")?;
    let fixture_dir = arguments.next().ok_or("missing fixture directory")?;
    let output_dir = arguments.next().ok_or("missing output directory")?;
    if arguments.next().is_some() {
        return Err("unexpected extra argument".into());
    }
    miniflow_flowlog_batch_corpus::run(&fixture, Path::new(&fixture_dir), Path::new(&output_dir))
}
