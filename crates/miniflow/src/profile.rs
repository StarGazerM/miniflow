//! Timely operator profiling for generated `MiniFlow` programs.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::io::Write;
use std::rc::Rc;
use std::time::Duration;

use timely::logging::StartStop;
use timely::logging::TimelyEvent;
use timely::logging::TimelyEventBuilder;
use timely::worker::Worker;

#[derive(Default)]
struct TimeStats {
    total_active: Duration,
    activations: u64,
    current_start: Option<Duration>,
}

#[derive(Default)]
struct OperatorMetrics {
    name: String,
    address: Vec<usize>,
    time: TimeStats,
}

/// A worker-local profiling session.
pub struct Session {
    worker: usize,
    operators: Rc<RefCell<HashMap<usize, OperatorMetrics>>>,
}

/// Register Timely event logging and write the generated static plan.
#[must_use]
pub fn install(worker: &Worker, plan: &str) -> Session {
    let operators = Rc::new(RefCell::new(HashMap::<usize, OperatorMetrics>::new()));
    let event_operators = Rc::clone(&operators);

    if worker.index() == 0
        && let Err(error) = prepare_output(plan)
    {
        eprintln!("miniflow profiling: failed to prepare program_log: {error}");
    }
    if let Some(mut registry) = worker.log_register() {
        registry.insert::<TimelyEventBuilder, _>("timely", move |_batch_time, data| {
            let Some(data) = data else {
                return;
            };
            for (timestamp, event) in data.iter() {
                match event {
                    TimelyEvent::Operates(operation) => {
                        let mut metrics = event_operators.borrow_mut();
                        let entry = metrics.entry(operation.id).or_default();
                        entry.name.clone_from(&operation.name);
                        entry.address.clone_from(&operation.addr);
                    }
                    TimelyEvent::Schedule(schedule) => {
                        let mut metrics = event_operators.borrow_mut();
                        let time = &mut metrics.entry(schedule.id).or_default().time;
                        match schedule.start_stop {
                            StartStop::Start => time.current_start = Some(*timestamp),
                            StartStop::Stop => {
                                if let Some(start) = time.current_start.take() {
                                    time.total_active +=
                                        timestamp.checked_sub(start).unwrap_or(Duration::ZERO);
                                    time.activations += 1;
                                }
                            }
                        }
                    }
                    TimelyEvent::Channels(_)
                    | TimelyEvent::Messages(_)
                    | TimelyEvent::PushProgress(_)
                    | TimelyEvent::Shutdown(_)
                    | TimelyEvent::CommChannels(_)
                    | TimelyEvent::Park(_)
                    | TimelyEvent::Text(_) => {}
                }
            }
        });
    } else {
        eprintln!("miniflow profiling: Timely log registry unavailable");
    }

    Session {
        worker: worker.index(),
        operators,
    }
}

impl Session {
    /// Write this worker's operator timing table.
    ///
    /// # Errors
    ///
    /// Returns an I/O error if the metrics directory or output file cannot be
    /// written.
    pub fn finish(self) -> io::Result<()> {
        let path = format!(
            "program_log/metrics/operators_worker_t0_{}.log",
            self.worker
        );
        let mut output = io::BufWriter::new(fs::File::create(path)?);
        writeln!(
            output,
            "{:<20} {:<6} {:<11} name",
            "addr", "acts", "active_ms"
        )?;

        let metrics = self.operators.borrow();
        let mut rows = metrics.values().collect::<Vec<_>>();
        rows.sort_by(|left, right| left.address.cmp(&right.address));
        for row in rows {
            writeln!(
                output,
                "{:<20} {:<6} {:<11.3} {}",
                format_address(&row.address),
                row.time.activations,
                row.time.total_active.as_secs_f64() * 1000.0,
                row.name
            )?;
        }
        output.flush()
    }
}

fn prepare_output(plan: &str) -> io::Result<()> {
    fs::create_dir_all("program_log/metrics")?;
    fs::write("program_log/ops.json", plan)
}

fn format_address(address: &[usize]) -> String {
    let cells = address.iter().map(usize::to_string).collect::<Vec<_>>();
    format!("[{}]", cells.join(", "))
}
