use perf_event::events::{Cache, CacheId, CacheOp, CacheResult, Dynamic, Hardware, Software};
use perf_event::{Builder, Counter, Group};
use perf_event_data::ReadFormat;
use std::collections::HashMap;
use std::error::Error;
use std::time::Instant;

pub trait Bundle {
    fn new(cpus: &[usize]) -> Result<Self, Box<dyn Error>>
    where
        Self: Sized;
    fn enable(&mut self) -> Result<(), Box<dyn Error>>;
    fn disable(&mut self) -> Result<(), Box<dyn Error>>;
    fn reset(&mut self) -> Result<(), Box<dyn Error>>;
    fn read(&mut self) -> Result<HashMap<String, String>, Box<dyn Error>>;
}

pub struct BundleConfig {
    pub rapl: bool,
    pub misses: bool,
    pub cstates: bool,
    pub cycles: bool,
}

impl BundleConfig {
    pub fn create_bundles(&self, cpus: &[usize]) -> Result<Vec<Box<dyn Bundle>>, Box<dyn Error>> {
        let mut bundles: Vec<Box<dyn Bundle>> = vec![];

        bundles.push(Box::new(TimeBundle::new(&[])?));

        if self.rapl {
            bundles.push(Box::new(RaplBundle::new(&[])?));
        }
        if self.cycles {
            bundles.push(Box::new(CyclesBundle::new(cpus)?));
        }
        if self.misses {
            bundles.push(Box::new(MissesBundle::new(cpus)?));
        }
        if self.cstates {
            bundles.push(Box::new(CStateBundle::new(cpus)?));
        }

        Ok(bundles)
    }
}

pub struct TimeBundle {
    start_time: Option<Instant>,
}

impl TimeBundle {
    pub const COLUMNS: &'static [&'static str] = &["time"];
}

impl Bundle for TimeBundle {
    fn new(_cpus: &[usize]) -> Result<Self, Box<dyn Error>> {
        Ok(Self { start_time: None })
    }

    fn enable(&mut self) -> Result<(), Box<dyn Error>> {
        self.start_time = Some(Instant::now());
        Ok(())
    }

    fn disable(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn reset(&mut self) -> Result<(), Box<dyn Error>> {
        self.start_time = None;
        Ok(())
    }

    fn read(&mut self) -> Result<HashMap<String, String>, Box<dyn Error>> {
        let mut measurements = HashMap::new();
        if let Some(start) = self.start_time {
            measurements.insert("time".to_string(), start.elapsed().as_micros().to_string());
        }
        Ok(measurements)
    }
}

struct RaplCounter {
    counter: Counter,
    scale: f64,
}

pub struct RaplBundle {
    group: Group,
    counters: HashMap<&'static str, RaplCounter>,
}

impl RaplBundle {
    pub const COLUMNS: &'static [&'static str] = &["pkg", "cores", "gpu", "ram", "psys"];
}

impl Bundle for RaplBundle {
    fn new(_cpus: &[usize]) -> Result<Self, Box<dyn Error>> {
        const RAPL_EVENTS: &[&str] = &[
            "energy-pkg",
            "energy-cores",
            "energy-gpu",
            "energy-psys",
            "energy-ram",
        ];

        let mut group = Builder::new(Software::DUMMY)
            .read_format(ReadFormat::GROUP | ReadFormat::TOTAL_TIME_RUNNING)
            .one_cpu(0)
            .any_pid()
            .exclude_hv(false)
            .exclude_kernel(false)
            .build_group()?;

        let mut counters = HashMap::new();

        for &event_name in RAPL_EVENTS {
            if let Ok(mut builder) = Dynamic::builder("power") {
                if builder.event(event_name).is_ok() {
                    if let Ok(Some(scale)) = builder.scale() {
                        if let Ok(built_event) = builder.build() {
                            if let Ok(counter) = Builder::new(built_event)
                                .one_cpu(0)
                                .any_pid()
                                .exclude_hv(false)
                                .exclude_kernel(false)
                                .build_with_group(&mut group)
                            {
                                counters.insert(event_name, RaplCounter { counter, scale });
                            }
                        }
                    }
                }
            }
        }

        Ok(Self { group, counters })
    }

    fn enable(&mut self) -> Result<(), Box<dyn Error>> {
        self.group.enable()?;
        Ok(())
    }

    fn disable(&mut self) -> Result<(), Box<dyn Error>> {
        self.group.disable()?;
        Ok(())
    }

    fn reset(&mut self) -> Result<(), Box<dyn Error>> {
        self.group.reset()?;
        Ok(())
    }

    fn read(&mut self) -> Result<HashMap<String, String>, Box<dyn Error>> {
        let mut measurements = HashMap::new();
        for (name, rapl_counter) in &mut self.counters {
            let raw = rapl_counter.counter.read()?;
            let scaled = raw as f64 * rapl_counter.scale;
            let key = match *name {
                "energy-pkg" => "pkg",
                "energy-cores" => "cores",
                "energy-gpu" => "gpu",
                "energy-ram" => "ram",
                "energy-psys" => "psys",
                other => other,
            };
            measurements.insert(key.to_string(), format!("{:.3}", scaled));
        }
        Ok(measurements)
    }
}

pub struct CyclesBundle {
    counters: Vec<Counter>,
}

impl CyclesBundle {
    pub const COLUMNS: &'static [&'static str] = &["cycles"];
}

impl Bundle for CyclesBundle {
    fn new(cpus: &[usize]) -> Result<Self, Box<dyn Error>> {
        let counters = cpus
            .iter()
            .filter_map(|&cpu| {
                Builder::new(Hardware::CPU_CYCLES)
                    .one_cpu(cpu)
                    .any_pid()
                    .exclude_kernel(false)
                    .exclude_hv(false)
                    .build()
                    .ok()
            })
            .collect();
        Ok(Self { counters })
    }

    fn enable(&mut self) -> Result<(), Box<dyn Error>> {
        for counter in &mut self.counters {
            counter.enable()?;
        }
        Ok(())
    }

    fn disable(&mut self) -> Result<(), Box<dyn Error>> {
        for counter in &mut self.counters {
            counter.disable()?;
        }
        Ok(())
    }

    fn reset(&mut self) -> Result<(), Box<dyn Error>> {
        for counter in &mut self.counters {
            counter.reset()?;
        }
        Ok(())
    }

    fn read(&mut self) -> Result<HashMap<String, String>, Box<dyn Error>> {
        let total: u64 = self
            .counters
            .iter_mut()
            .map(|c| c.read())
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .sum();
        let mut measurements = HashMap::new();
        measurements.insert("cycles".to_string(), total.to_string());
        Ok(measurements)
    }
}

pub struct MissesBundle {
    counters: HashMap<&'static str, Vec<Counter>>,
}

impl MissesBundle {
    pub const COLUMNS: &'static [&'static str] =
        &["l1d_misses", "l1i_misses", "llc_misses", "branch_misses"];
}

const L1D_MISS: Cache = Cache {
    which: CacheId::L1D,
    operation: CacheOp::READ,
    result: CacheResult::MISS,
};
const L1I_MISS: Cache = Cache {
    which: CacheId::L1I,
    operation: CacheOp::READ,
    result: CacheResult::MISS,
};
const LLC_MISS: Cache = Cache {
    which: CacheId::LL,
    operation: CacheOp::READ,
    result: CacheResult::MISS,
};

enum MissEvent {
    L1d,
    L1i,
    Llc,
    Branch,
}

impl MissEvent {
    fn name(&self) -> &'static str {
        match self {
            MissEvent::L1d => "l1d_misses",
            MissEvent::L1i => "l1i_misses",
            MissEvent::Llc => "llc_misses",
            MissEvent::Branch => "branch_misses",
        }
    }

    fn build_counter(&self, cpu: usize) -> Result<Counter, std::io::Error> {
        match self {
            MissEvent::L1d => Builder::new(L1D_MISS),
            MissEvent::L1i => Builder::new(L1I_MISS),
            MissEvent::Llc => Builder::new(LLC_MISS),
            MissEvent::Branch => Builder::new(Hardware::BRANCH_MISSES),
        }
        .one_cpu(cpu)
        .any_pid()
        .exclude_kernel(false)
        .exclude_hv(false)
        .build()
    }
}

impl Bundle for MissesBundle {
    fn new(cpus: &[usize]) -> Result<Self, Box<dyn Error>> {
        let events = [
            MissEvent::L1d,
            MissEvent::L1i,
            MissEvent::Llc,
            MissEvent::Branch,
        ];
        let mut counters = HashMap::new();

        for event in &events {
            let cpu_counters: Vec<Counter> = cpus
                .iter()
                .filter_map(|&cpu| event.build_counter(cpu).ok())
                .collect();
            if !cpu_counters.is_empty() {
                counters.insert(event.name(), cpu_counters);
            }
        }

        Ok(Self { counters })
    }

    fn enable(&mut self) -> Result<(), Box<dyn Error>> {
        for cpu_counters in self.counters.values_mut() {
            for counter in cpu_counters {
                counter.enable()?;
            }
        }
        Ok(())
    }

    fn disable(&mut self) -> Result<(), Box<dyn Error>> {
        for cpu_counters in self.counters.values_mut() {
            for counter in cpu_counters {
                counter.disable()?;
            }
        }
        Ok(())
    }

    fn reset(&mut self) -> Result<(), Box<dyn Error>> {
        for cpu_counters in self.counters.values_mut() {
            for counter in cpu_counters {
                counter.reset()?;
            }
        }
        Ok(())
    }

    fn read(&mut self) -> Result<HashMap<String, String>, Box<dyn Error>> {
        let mut measurements = HashMap::new();
        for (name, cpu_counters) in &mut self.counters {
            let total: u64 = cpu_counters
                .iter_mut()
                .map(|c| c.read())
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .sum();
            measurements.insert(name.to_string(), total.to_string());
        }
        Ok(measurements)
    }
}

struct CStateCounter {
    counter: Counter,
    event_name: &'static str,
}

pub struct CStateBundle {
    core_counters: Vec<CStateCounter>,
    pkg_counters: Vec<CStateCounter>,
}

impl CStateBundle {
    pub const COLUMNS: &'static [&'static str] = &[
        "c1_core_residency",
        "c3_core_residency",
        "c6_core_residency",
        "c7_core_residency",
        "c2_pkg_residency",
        "c3_pkg_residency",
        "c6_pkg_residency",
        "c8_pkg_residency",
        "c10_pkg_residency",
    ];
}

const CORE_EVENTS: &[&str] = &[
    "c1-residency",
    "c3-residency",
    "c6-residency",
    "c7-residency",
];
const PKG_EVENTS: &[&str] = &[
    "c2-residency",
    "c3-residency",
    "c6-residency",
    "c8-residency",
    "c10-residency",
];

fn build_dynamic_counter(pmu: &str, event_name: &str, cpu: usize) -> Option<Counter> {
    let mut builder = Dynamic::builder(pmu).ok()?;
    builder.event(event_name).ok()?;
    let built = builder.build().ok()?;
    Builder::new(built)
        .one_cpu(cpu)
        .any_pid()
        .exclude_kernel(false)
        .exclude_hv(false)
        .build()
        .ok()
}

impl Bundle for CStateBundle {
    fn new(cpus: &[usize]) -> Result<Self, Box<dyn Error>> {
        let mut core_counters = Vec::new();
        let mut pkg_counters = Vec::new();

        for &event_name in CORE_EVENTS {
            for &cpu in cpus {
                if let Some(counter) = build_dynamic_counter("cstate_core", event_name, cpu) {
                    core_counters.push(CStateCounter {
                        counter,
                        event_name,
                    });
                }
            }
        }

        for &event_name in PKG_EVENTS {
            if let Some(counter) = build_dynamic_counter("cstate_pkg", event_name, 0) {
                pkg_counters.push(CStateCounter {
                    counter,
                    event_name,
                });
            }
        }

        Ok(Self {
            core_counters,
            pkg_counters,
        })
    }

    fn enable(&mut self) -> Result<(), Box<dyn Error>> {
        for c in self
            .core_counters
            .iter_mut()
            .chain(self.pkg_counters.iter_mut())
        {
            c.counter.enable()?;
        }
        Ok(())
    }

    fn disable(&mut self) -> Result<(), Box<dyn Error>> {
        for c in self
            .core_counters
            .iter_mut()
            .chain(self.pkg_counters.iter_mut())
        {
            c.counter.disable()?;
        }
        Ok(())
    }

    fn reset(&mut self) -> Result<(), Box<dyn Error>> {
        for c in self
            .core_counters
            .iter_mut()
            .chain(self.pkg_counters.iter_mut())
        {
            c.counter.reset()?;
        }
        Ok(())
    }

    fn read(&mut self) -> Result<HashMap<String, String>, Box<dyn Error>> {
        let mut aggregated: HashMap<&'static str, u64> = HashMap::new();

        for c in &mut self.core_counters {
            *aggregated.entry(c.event_name).or_insert(0) += c.counter.read()?;
        }

        let mut measurements = HashMap::new();
        for (name, value) in &aggregated {
            let key = match *name {
                "c1-residency" => "c1_core_residency",
                "c3-residency" => "c3_core_residency",
                "c6-residency" => "c6_core_residency",
                "c7-residency" => "c7_core_residency",
                other => other,
            };
            measurements.insert(key.to_string(), value.to_string());
        }

        for c in &mut self.pkg_counters {
            let key = match c.event_name {
                "c2-residency" => "c2_pkg_residency",
                "c3-residency" => "c3_pkg_residency",
                "c6-residency" => "c6_pkg_residency",
                "c8-residency" => "c8_pkg_residency",
                "c10-residency" => "c10_pkg_residency",
                other => other,
            };
            measurements.insert(key.to_string(), c.counter.read()?.to_string());
        }

        Ok(measurements)
    }
}
