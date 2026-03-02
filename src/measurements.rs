use crate::bundles::{
    Bundle, BundleConfig, CStateBundle, CyclesBundle, MissesBundle, RaplBundle, TimeBundle,
};
use std::collections::HashMap;
use std::ffi::{CStr, c_char};
use std::path::PathBuf;

pub struct MeasurementContext {
    bundles: Vec<Box<dyn Bundle>>,
    output_path: PathBuf,
}

fn column_order() -> Vec<&'static str> {
    [
        TimeBundle::COLUMNS,
        RaplBundle::COLUMNS,
        CyclesBundle::COLUMNS,
        MissesBundle::COLUMNS,
        CStateBundle::COLUMNS,
        &["ended"],
    ]
    .concat()
}

fn parse_config(metrics: *const c_char) -> BundleConfig {
    let s = unsafe { CStr::from_ptr(metrics) }.to_str().unwrap_or("");

    let flags: Vec<&str> = s.split(',').map(str::trim).collect();

    BundleConfig {
        rapl: flags.contains(&"rapl"),
        misses: flags.contains(&"misses"),
        cstates: flags.contains(&"cstates"),
        cycles: flags.contains(&"cycles"),
    }
}

fn resolve_cpus() -> Vec<usize> {
    affinity::get_thread_affinity().unwrap_or_else(|_| (0..num_cpus::get()).collect())
}

pub fn measure_start(metrics: *const c_char) -> *mut MeasurementContext {
    let config = parse_config(metrics);
    let cpus = resolve_cpus();

    let mut bundles = match config.create_bundles(&cpus) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("libgreen: failed to initialise bundles: {}", e);
            return std::ptr::null_mut();
        }
    };

    for bundle in &mut bundles {
        if let Err(e) = bundle.reset().and_then(|_| bundle.enable()) {
            eprintln!("libgreen: failed to enable counters: {}", e);
            return std::ptr::null_mut();
        }
    }

    let output_path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    Box::into_raw(Box::new(MeasurementContext {
        bundles,
        output_path,
    }))
}

pub fn measure_stop(state: *mut MeasurementContext) {
    if state.is_null() {
        eprintln!("libgreen: measure_stop called with null context");
        return;
    }

    let mut state = unsafe { Box::from_raw(state) };

    for bundle in &mut state.bundles {
        if let Err(e) = bundle.disable() {
            eprintln!("libgreen: failed to disable counters: {}", e);
        }
    }

    let mut data: HashMap<String, String> = HashMap::new();
    for bundle in &mut state.bundles {
        match bundle.read() {
            Ok(readings) => data.extend(readings),
            Err(e) => eprintln!("libgreen: failed to read bundle: {}", e),
        }
    }

    data.insert(
        "ended".to_string(),
        chrono::Utc::now().timestamp_micros().to_string(),
    );

    if let Err(e) = write_to_csv(&data, &state.output_path) {
        eprintln!("libgreen: failed to write measurements: {}", e);
    }
}

fn write_to_csv(
    data: &HashMap<String, String>,
    output_path: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let csv_path = output_path.join("measurements.csv");
    let file_exists = csv_path.exists();
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .append(true)
        .open(&csv_path)?;

    let mut wtr = csv::WriterBuilder::new()
        .has_headers(!file_exists)
        .from_writer(file);

    let headers: Vec<&str> = column_order()
        .into_iter()
        .filter(|&k| data.contains_key(k))
        .collect();

    if !file_exists {
        wtr.write_record(&headers)?;
    }

    wtr.write_record(headers.iter().map(|&k| data[k].as_str()))?;
    wtr.flush()?;
    Ok(())
}
