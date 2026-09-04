//! Reproducible core-latency benchmark for a weighted attention TSV graph.

use spectral_pruner::{PrunerWorkspace, TauSpectralPruner, Topology};
use std::env;
use std::fs;
use std::time::Instant;

struct Args {
    edge_path: String,
    nodes: usize,
    system_start: usize,
    system_end: usize,
    warmup: usize,
    runs: usize,
    max_iterations: usize,
    tolerance: f64,
}

fn parse_value<T: std::str::FromStr>(name: &str, raw: Option<String>) -> T {
    raw.unwrap_or_else(|| panic!("{} requires a value", name))
        .parse()
        .unwrap_or_else(|_| panic!("invalid value for {}", name))
}

fn parse_args() -> Args {
    let mut values = env::args().skip(1);
    let mut edge_path = None;
    let mut nodes = None;
    let mut system_start = None;
    let mut system_end = None;
    let mut warmup = 100;
    let mut runs = 2_000;
    let mut max_iterations = 10_000;
    let mut tolerance = 1e-9;
    while let Some(arg) = values.next() {
        match arg.as_str() {
            "--nodes" => nodes = Some(parse_value("--nodes", values.next())),
            "--system-start" => system_start = Some(parse_value("--system-start", values.next())),
            "--system-end" => system_end = Some(parse_value("--system-end", values.next())),
            "--warmup" => warmup = parse_value("--warmup", values.next()),
            "--runs" => runs = parse_value("--runs", values.next()),
            "--max-iterations" => max_iterations = parse_value("--max-iterations", values.next()),
            "--tolerance" => tolerance = parse_value("--tolerance", values.next()),
            "--help" | "-h" => {
                println!(concat!(
                    "Usage: attention_tsv_benchmark --nodes N --system-start I --system-end J ",
                    "[--warmup N] [--runs N] [--max-iterations N] [--tolerance X] EDGES.tsv\n",
                    "Defaults: warmup=100 runs=2000 max-iterations=10000 tolerance=1e-9\n",
                    "Measures the core audit only; reports convergence alongside latency."
                ));
                std::process::exit(0);
            }
            _ if arg.starts_with('-') => panic!("unknown option: {}", arg),
            _ => {
                assert!(edge_path.replace(arg).is_none(), "supply one edge TSV");
            }
        }
    }
    Args {
        edge_path: edge_path.expect("missing edge TSV"),
        nodes: nodes.expect("missing --nodes"),
        system_start: system_start.expect("missing --system-start"),
        system_end: system_end.expect("missing --system-end"),
        warmup,
        runs,
        max_iterations,
        tolerance,
    }
}

fn load_topology(args: &Args) -> Topology {
    let input = fs::read_to_string(&args.edge_path).expect("edge TSV must be readable");
    let mut topology = Topology::new(args.nodes);
    for (line_number, line) in input.lines().enumerate() {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let columns: Vec<_> = line.split_whitespace().collect();
        assert_eq!(columns.len(), 3, "malformed edge row {}", line_number + 1);
        let source: usize = columns[0].parse().expect("invalid source");
        let target: usize = columns[1].parse().expect("invalid target");
        assert!(
            source < args.nodes && target < args.nodes,
            "edge row {} is outside the declared node count",
            line_number + 1
        );
        topology.add_weighted_edge(source, target, columns[2].parse().expect("invalid weight"));
    }
    topology
}

fn percentile(samples: &[u128], fraction: f64) -> f64 {
    let index = ((samples.len() - 1) as f64 * fraction).round() as usize;
    samples[index] as f64 / 1_000.0
}

fn main() {
    let args = parse_args();
    assert!(args.runs > 0, "--runs must be positive");
    let topology = load_topology(&args);
    let pruner = TauSpectralPruner::builder()
        .system_start_idx(args.system_start)
        .max_iterations(args.max_iterations)
        .tolerance(args.tolerance)
        .spectral_only()
        .try_build()
        .expect("invalid benchmark solver settings");
    let mut workspace = PrunerWorkspace::with_capacity(args.nodes, topology.edge_count());

    for _ in 0..args.warmup {
        pruner
            .prune_with_workspace(&topology, args.system_end, &mut workspace)
            .unwrap();
    }

    let mut samples = Vec::with_capacity(args.runs);
    let mut checksum = 0.0;
    let mut converged_runs = 0;
    let mut total_iterations = 0;
    for _ in 0..args.runs {
        let started = Instant::now();
        let result = pruner
            .prune_with_workspace(&topology, args.system_end, &mut workspace)
            .unwrap();
        samples.push(started.elapsed().as_nanos());
        checksum += result.connectivity_score;
        converged_runs += usize::from(result.diagnostics.solver_converged);
        total_iterations += result.diagnostics.solver_iterations;
    }
    samples.sort_unstable();
    let mean_us = samples.iter().sum::<u128>() as f64 / args.runs as f64 / 1_000.0;

    println!(
        concat!(
            "{{\"schema_version\":1,\"nodes\":{},\"edges\":{},\"warmup\":{},",
            "\"runs\":{},\"p50_us\":{:.6},\"p95_us\":{:.6},",
            "\"p99_us\":{:.6},\"mean_us\":{:.6},\"checksum\":{},",
            "\"max_iterations\":{},\"tolerance\":{},\"converged_runs\":{},\"mean_iterations\":{}}}"
        ),
        args.nodes,
        topology.edge_count(),
        args.warmup,
        args.runs,
        percentile(&samples, 0.50),
        percentile(&samples, 0.95),
        percentile(&samples, 0.99),
        mean_us,
        if checksum.is_finite() {
            checksum.to_string()
        } else {
            "null".to_string()
        },
        args.max_iterations,
        args.tolerance,
        converged_runs,
        total_iterations as f64 / args.runs as f64
    );
}
