use spectral_pruner::{PrunerDiagnostics, TauSpectralPruner, Topology};
use std::env;
use std::fs;
use std::io::{self, Read};
use std::process;

#[derive(Debug)]
struct Cli {
    edge_path: String,
    nodes: usize,
    system_start: usize,
    system_end: usize,
    sinks: Vec<usize>,
    tau: f64,
    max_iterations: usize,
    tolerance: f64,
    threat_threshold: f64,
    connectivity_threshold: Option<f64>,
    instruction_threshold: f64,
    density_enabled: bool,
    neglect_enabled: bool,
    tripwire_enabled: bool,
}

fn usage() -> &'static str {
    "Usage: spectral-pruner-audit --nodes N --system-start N --system-end N [options] EDGE_TSV\n\
     \n\
     EDGE_TSV contains one undirected weighted edge per line: source<TAB>target<TAB>weight.\n\
     Pass - to read edges from standard input.\n\
     Empty lines and lines beginning with # are ignored.\n\
     \n\
     Options:\n\
       --sink N                 Mark a sink node; repeat as needed\n\
       --tau F                  Fiedler partition threshold (default: 0.0)\n\
       --max-iterations N       Solver iteration budget (default: 10000)\n\
       --tolerance F            Solver convergence tolerance (default: 1e-9)\n\
       --threat-threshold F     Density-ratio threshold (default: 2.0)\n\
       --connectivity-threshold F  Block at or below this lambda_2 (disabled by default)\n\
       --instruction-threshold F   Instruction-connection floor (default: 0.1)\n\
       --spectral-only          Disable non-spectral policy heuristics\n\
       --disable-density        Disable the density-ratio trigger\n\
       --disable-neglect        Disable the instruction-neglect trigger\n\
       --disable-tripwire       Disable the single-token trigger\n\
       --version                Print the package version\n\
       --help                   Print this help"
}

fn parse_value<T: std::str::FromStr>(name: &str, value: Option<String>) -> Result<T, String> {
    let raw = value.ok_or_else(|| format!("{} requires a value", name))?;
    raw.parse::<T>()
        .map_err(|_| format!("invalid value for {}: {}", name, raw))
}

fn parse_cli() -> Result<Cli, String> {
    let mut args = env::args().skip(1);
    let mut nodes = None;
    let mut system_start = None;
    let mut system_end = None;
    let mut sinks = Vec::new();
    let mut tau = 0.0;
    let mut max_iterations = 10_000;
    let mut tolerance = 1e-9;
    let mut threat_threshold = 2.0;
    let mut connectivity_threshold = None;
    let mut instruction_threshold = 0.1;
    let mut density_enabled = true;
    let mut neglect_enabled = true;
    let mut tripwire_enabled = true;
    let mut edge_path = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--nodes" => nodes = Some(parse_value("--nodes", args.next())?),
            "--system-start" => system_start = Some(parse_value("--system-start", args.next())?),
            "--system-end" => system_end = Some(parse_value("--system-end", args.next())?),
            "--sink" => sinks.push(parse_value("--sink", args.next())?),
            "--tau" => tau = parse_value("--tau", args.next())?,
            "--max-iterations" => max_iterations = parse_value("--max-iterations", args.next())?,
            "--tolerance" => tolerance = parse_value("--tolerance", args.next())?,
            "--threat-threshold" => {
                threat_threshold = parse_value("--threat-threshold", args.next())?
            }
            "--connectivity-threshold" => {
                connectivity_threshold = Some(parse_value("--connectivity-threshold", args.next())?)
            }
            "--instruction-threshold" => {
                instruction_threshold = parse_value("--instruction-threshold", args.next())?
            }
            "--spectral-only" => {
                density_enabled = false;
                neglect_enabled = false;
                tripwire_enabled = false;
            }
            "--disable-density" => density_enabled = false,
            "--disable-neglect" => neglect_enabled = false,
            "--disable-tripwire" => tripwire_enabled = false,
            "--version" | "-V" => {
                println!("spectral-pruner-audit {}", env!("CARGO_PKG_VERSION"));
                process::exit(0);
            }
            "--help" | "-h" => {
                println!("{}", usage());
                process::exit(0);
            }
            "-" => {
                if edge_path.replace(arg).is_some() {
                    return Err("only one edge file may be supplied".to_string());
                }
            }
            _ if arg.starts_with('-') => return Err(format!("unknown option: {}", arg)),
            _ => {
                if edge_path.replace(arg).is_some() {
                    return Err("only one edge file may be supplied".to_string());
                }
            }
        }
    }

    Ok(Cli {
        edge_path: edge_path.ok_or_else(|| "missing EDGE_TSV path".to_string())?,
        nodes: nodes.ok_or_else(|| "missing --nodes".to_string())?,
        system_start: system_start.ok_or_else(|| "missing --system-start".to_string())?,
        system_end: system_end.ok_or_else(|| "missing --system-end".to_string())?,
        sinks,
        tau,
        max_iterations,
        tolerance,
        threat_threshold,
        connectivity_threshold,
        instruction_threshold,
        density_enabled,
        neglect_enabled,
        tripwire_enabled,
    })
}

fn load_topology(cli: &Cli) -> Result<Topology, String> {
    let input = if cli.edge_path == "-" {
        let mut input = String::new();
        io::stdin()
            .read_to_string(&mut input)
            .map_err(|error| format!("failed to read standard input: {}", error))?;
        input
    } else {
        fs::read_to_string(&cli.edge_path)
            .map_err(|error| format!("failed to read {}: {}", cli.edge_path, error))?
    };
    let mut topology = Topology::new(cli.nodes);

    for (line_idx, line) in input.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let columns: Vec<&str> = line.split_whitespace().collect();
        if columns.len() != 3 {
            return Err(format!(
                "{}:{} expected three columns: source target weight",
                cli.edge_path,
                line_idx + 1
            ));
        }
        let source = columns[0]
            .parse::<usize>()
            .map_err(|_| format!("{}:{} invalid source", cli.edge_path, line_idx + 1))?;
        let target = columns[1]
            .parse::<usize>()
            .map_err(|_| format!("{}:{} invalid target", cli.edge_path, line_idx + 1))?;
        let weight = columns[2]
            .parse::<f64>()
            .map_err(|_| format!("{}:{} invalid weight", cli.edge_path, line_idx + 1))?;
        if source >= cli.nodes || target >= cli.nodes {
            return Err(format!(
                "{}:{} endpoint outside {}-node topology",
                cli.edge_path,
                line_idx + 1,
                cli.nodes
            ));
        }
        topology.add_weighted_edge(source, target, weight);
    }

    for &sink in &cli.sinks {
        if sink >= cli.nodes {
            return Err(format!(
                "sink {} is outside {}-node topology",
                sink, cli.nodes
            ));
        }
        topology.add_sink(sink);
    }
    Ok(topology)
}

fn json_float(value: f64) -> String {
    if value.is_finite() {
        value.to_string()
    } else {
        "null".to_string()
    }
}

fn json_usize_array(values: &[usize]) -> String {
    let body = values
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(",");
    format!("[{}]", body)
}

fn diagnostics_json(d: &PrunerDiagnostics) -> String {
    let ratio_status = if d.density_ratio.is_infinite() {
        "infinite"
    } else if d.density_ratio.is_nan() {
        "nan"
    } else {
        "finite"
    };
    let possible_ratio_status = if d.possible_edge_density_ratio.is_infinite() {
        "infinite"
    } else if d.possible_edge_density_ratio.is_nan() {
        "nan"
    } else {
        "finite"
    };
    format!(
        concat!(
            "{{\"boundary_configuration_valid\":{},",
            "\"solver_converged\":{},\"solver_iterations\":{},",
            "\"relative_residual\":{},\"numerical_failure_triggered\":{},",
            "\"island_node_count\":{},\"system_node_count\":{},",
            "\"internal_weight\":{},\"system_weight\":{},",
            "\"partition_cut_weight\":{},\"island_volume\":{},\"conductance\":{},",
            "\"internal_density\":{},\"boundary_density\":{},",
            "\"possible_edge_density_ratio\":{},",
            "\"possible_edge_density_ratio_status\":\"{}\",",
            "\"density_ratio\":{},\"density_ratio_status\":\"{}\",",
            "\"instruction_connection\":{},\"connectivity_triggered\":{},",
            "\"density_triggered\":{},",
            "\"instruction_neglect_triggered\":{},\"single_token_triggered\":{}}}"
        ),
        d.boundary_configuration_valid,
        d.solver_converged,
        d.solver_iterations,
        d.relative_residual
            .map(json_float)
            .unwrap_or_else(|| "null".to_string()),
        d.numerical_failure_triggered,
        d.island_node_count,
        d.system_node_count,
        json_float(d.internal_weight),
        json_float(d.system_weight),
        json_float(d.partition_cut_weight),
        json_float(d.island_volume),
        json_float(d.conductance),
        json_float(d.internal_density),
        json_float(d.boundary_density),
        json_float(d.possible_edge_density_ratio),
        possible_ratio_status,
        json_float(d.density_ratio),
        ratio_status,
        json_float(d.instruction_connection),
        d.connectivity_triggered,
        d.density_triggered,
        d.instruction_neglect_triggered,
        d.single_token_triggered
    )
}

fn run() -> Result<(), String> {
    let cli = parse_cli()?;
    let topology = load_topology(&cli)?;
    let mut builder = TauSpectralPruner::builder()
        .tau(cli.tau)
        .max_iterations(cli.max_iterations)
        .tolerance(cli.tolerance)
        .threat_threshold(cli.threat_threshold)
        .system_start_idx(cli.system_start)
        .instruction_connection_threshold(cli.instruction_threshold)
        .density_ratio_enabled(cli.density_enabled)
        .instruction_neglect_enabled(cli.neglect_enabled)
        .single_token_tripwire_enabled(cli.tripwire_enabled);
    if let Some(threshold) = cli.connectivity_threshold {
        builder = builder.connectivity_threshold(threshold);
    }
    let pruner = builder.try_build().map_err(|error| error.to_string())?;
    let result = pruner
        .prune(&topology, cli.system_end)
        .map_err(|error| error.to_string())?;

    println!(
        concat!(
            "{{\"schema_version\":1,\"action\":\"{}\",",
            "\"connectivity_score\":{},\"mainland_nodes\":{},",
            "\"island_nodes\":{},\"diagnostics\":{}}}"
        ),
        result.action,
        json_float(result.connectivity_score),
        json_usize_array(&result.mainland_nodes),
        json_usize_array(&result.island_nodes),
        diagnostics_json(&result.diagnostics)
    );
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {}\n\n{}", error, usage());
        process::exit(2);
    }
}
