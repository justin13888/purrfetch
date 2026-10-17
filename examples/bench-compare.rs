//! Portable end-to-end comparison benchmark for purr and other fetch tools.

use std::{
    collections::BTreeMap,
    env, fs, io,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

use crossterm::style::Stylize;
use serde::Serialize;
use serde_json::Value;

const WARMUP_RUNS: u32 = 5;
const MINIMUM_RUNS: u32 = 20;
const OUTPUT_MODE: &str = "pipe";
const FASTFETCH_STRUCTURE: &str = "Title:Separator:OS:Host:Kernel:Uptime:Packages:Shell:Display:DE:WM:WMTheme:Theme:Icons:\
     Terminal:TerminalFont:CPU:GPU:Memory";
const TRACE_RUNS: usize = 20;
/// Separate target directory so the traced build never replaces the binary hyperfine timed.
const TRACE_TARGET_DIRECTORY: &str = "target/bench-profile";
/// Soft budget for purr's median wall time; missing it warns rather than fails.
const SOFT_BUDGET_MS: f64 = 25.0;

#[derive(Debug)]
struct ToolSpec {
    name: &'static str,
    executables: Vec<String>,
    arguments: Vec<String>,
    version_arguments: &'static [&'static str],
}

#[derive(Debug)]
struct AvailableTool {
    name: &'static str,
    version: String,
    arguments: Vec<String>,
    command: String,
}

#[derive(Debug, Serialize)]
struct Measurements {
    purr_binary_bytes: u64,
    tools: Vec<ToolSummary>,
    trace_runs: usize,
    spans: Vec<SpanTiming>,
    /// Why the span phase produced nothing; it never discards the hyperfine results.
    span_error: Option<String>,
    verdict: BudgetVerdict,
}

/// Per-tool hyperfine statistics; wall times in ms, peak RSS only from hyperfine 2 or newer.
#[derive(Debug, PartialEq, Serialize)]
struct ToolSummary {
    name: String,
    median_ms: f64,
    mean_ms: f64,
    stddev_ms: f64,
    min_ms: f64,
    max_ms: f64,
    peak_rss_median_bytes: Option<u64>,
}

#[derive(Debug, PartialEq, Serialize)]
struct SpanTiming {
    span: String,
    median_ms: f64,
    p95_ms: f64,
    samples: usize,
}

#[derive(Debug, PartialEq, Serialize)]
struct BudgetVerdict {
    purr_median_ms: f64,
    fastfetch_median_ms: Option<f64>,
    beats_fastfetch: Outcome,
    soft_budget_ms: f64,
    within_soft_budget: Outcome,
    neofetch: Option<NeofetchRatio>,
}

#[derive(Debug, PartialEq, Serialize)]
struct NeofetchRatio {
    tool: String,
    median_ms: f64,
    speedup: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
enum Outcome {
    Pass,
    Warn,
    Fail,
    Skipped,
    Info,
}

impl Outcome {
    fn label(self) -> &'static str {
        match self {
            Outcome::Pass => "PASS",
            Outcome::Warn => "WARN",
            Outcome::Fail => "FAIL",
            Outcome::Skipped => "SKIPPED",
            Outcome::Info => "INFO",
        }
    }
}

#[derive(Debug, Serialize)]
struct BenchmarkContext {
    platform: PlatformContext,
    host: BTreeMap<String, String>,
    sampling: SamplingContext,
    tools: Vec<ToolContext>,
    skipped: Vec<SkippedTool>,
}

#[derive(Debug, Serialize)]
struct PlatformContext {
    os: &'static str,
    family: &'static str,
    architecture: &'static str,
    logical_cpus: usize,
}

#[derive(Debug, Serialize)]
struct SamplingContext {
    warmup_runs: u32,
    minimum_runs: u32,
    output_mode: &'static str,
    timing_scope: &'static str,
}

#[derive(Debug, Serialize)]
struct ToolContext {
    name: &'static str,
    version: String,
    command: String,
}

#[derive(Debug, Serialize)]
struct SkippedTool {
    name: &'static str,
    reason: String,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{}", format!("Error: {error}").red());
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    validate_arguments()?;

    let repository_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    env::set_current_dir(&repository_root)
        .map_err(|error| format!("could not enter the repository root: {error}"))?;

    let scratch_directory = repository_root.join("target/bench-compare");
    fs::create_dir_all(&scratch_directory)
        .map_err(|error| format!("could not create benchmark scratch directory: {error}"))?;

    let macchina_config = scratch_directory.join("macchina.toml");
    fs::write(&macchina_config, "")
        .map_err(|error| format!("could not create the macchina configuration: {error}"))?;

    let temporary_json = scratch_directory.join("hyperfine.json");
    let temporary_markdown = scratch_directory.join("hyperfine.md");
    remove_if_present(&temporary_json)?;
    remove_if_present(&temporary_markdown)?;

    let result = execute_benchmark(
        &repository_root,
        &macchina_config,
        &temporary_json,
        &temporary_markdown,
    );

    let cleanup_result = [&macchina_config, &temporary_json, &temporary_markdown]
        .into_iter()
        .try_for_each(|path| remove_if_present(path));

    match (result, cleanup_result) {
        (Err(error), _) => Err(error),
        (Ok(()), Err(error)) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
    }
}

fn validate_arguments() -> Result<(), String> {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    match arguments.as_slice() {
        [] => Ok(()),
        [argument] if argument == "--help" || argument == "-h" => {
            println!(
                "Usage: cargo run --release --example bench-compare\n\n\
                 Builds purr and benchmarks every available supported fetch tool."
            );
            std::process::exit(0);
        }
        _ => Err(format!(
            "unexpected arguments: {}\nUsage: cargo run --release --example bench-compare",
            arguments.join(" ")
        )),
    }
}

fn execute_benchmark(
    repository_root: &Path,
    macchina_config: &Path,
    temporary_json: &Path,
    temporary_markdown: &Path,
) -> Result<(), String> {
    let hyperfine_version = required_hyperfine_version()?;

    println!("Building purr (release, locked)...");
    let build_status = Command::new("cargo")
        .args(["build", "--release", "--locked", "--bin", "purr"])
        .status()
        .map_err(|error| format!("could not start cargo: {error}"))?;
    if !build_status.success() {
        return Err(format!("cargo build failed with {build_status}"));
    }

    let purr_executable = purr_executable(env::consts::EXE_SUFFIX);
    let purr_spec = ToolSpec {
        name: "purr",
        executables: vec![purr_executable.clone()],
        arguments: vec!["--no-config".into(), "--stdout".into()],
        version_arguments: &["--version"],
    };
    let purr = preflight_tool(&purr_spec)
        .map_err(|reason| format!("the freshly built purr executable is unusable: {reason}"))?;

    let macchina_config = relative_path(repository_root, macchina_config)?;
    let specs = competitor_specs(&macchina_config);
    let mut tools = vec![purr];
    let mut skipped = Vec::new();

    for spec in &specs {
        match preflight_tool(spec) {
            Ok(tool) => tools.push(tool),
            Err(reason) => {
                eprintln!(
                    "{}",
                    format!("Warning: skipping {}: {reason}", spec.name).yellow()
                );
                skipped.push(SkippedTool {
                    name: spec.name,
                    reason,
                });
            }
        }
    }

    println!(
        "\nBenchmarking {} with {WARMUP_RUNS} warmups and at least {MINIMUM_RUNS} measured runs.",
        tools
            .iter()
            .map(|tool| tool.name)
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!("Each command's complete stdout is drained through a pipe.\n");

    run_hyperfine(&tools, temporary_json, temporary_markdown)?;

    let hyperfine_results = read_json(temporary_json)?;
    let summaries = hyperfine_summaries(&hyperfine_results);
    let verdict = evaluate_budget(&summaries)?;

    println!("\nBuilding traced purr (release, profile feature)...");
    let span_result = build_traced_purr(repository_root).and_then(|traced_purr| {
        println!("Collecting cold span timings over {TRACE_RUNS} runs...");
        measure_spans(
            &traced_purr,
            &tools[0].arguments,
            &repository_root.join("target/bench-compare/traces"),
        )
    });
    let (spans, span_error) = match span_result {
        Ok(spans) => (spans, None),
        Err(reason) => {
            eprintln!(
                "{}",
                format!("Warning: skipping span timings: {reason}").yellow()
            );
            (Vec::new(), Some(reason))
        }
    };

    let purr_binary_bytes = fs::metadata(&purr_executable)
        .map_err(|error| format!("could not stat {purr_executable}: {error}"))?
        .len();
    let measurements = Measurements {
        purr_binary_bytes,
        tools: summaries,
        trace_runs: TRACE_RUNS,
        spans,
        span_error,
        verdict,
    };
    print_verdict(&measurements.verdict);

    let context = BenchmarkContext {
        platform: PlatformContext {
            os: env::consts::OS,
            family: env::consts::FAMILY,
            architecture: env::consts::ARCH,
            logical_cpus: std::thread::available_parallelism()
                .map(usize::from)
                .unwrap_or(1),
        },
        host: collect_host_context(&purr_executable),
        sampling: SamplingContext {
            warmup_runs: WARMUP_RUNS,
            minimum_runs: MINIMUM_RUNS,
            output_mode: OUTPUT_MODE,
            timing_scope: "process completion after stdout is fully drained",
        },
        tools: tools
            .into_iter()
            .map(|tool| ToolContext {
                name: tool.name,
                version: tool.version,
                command: tool.command,
            })
            .collect(),
        skipped,
    };

    let json_output = repository_root.join("bench-results.json");
    let markdown_output = repository_root.join("bench-results.md");
    enrich_json(
        hyperfine_results,
        &json_output,
        &hyperfine_version,
        &context,
        &measurements,
    )?;
    enrich_markdown(
        temporary_markdown,
        &markdown_output,
        &hyperfine_version,
        &context,
        &measurements,
    )?;

    println!(
        "\nResults saved to {} and {}.",
        json_output.display(),
        markdown_output.display()
    );
    Ok(())
}

fn competitor_specs(macchina_config: &str) -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "fastfetch",
            executables: vec!["fastfetch".into()],
            arguments: vec![
                "--config".into(),
                "none".into(),
                "--pipe".into(),
                "--structure".into(),
                FASTFETCH_STRUCTURE.into(),
            ],
            version_arguments: &["--version"],
        },
        ToolSpec {
            name: "macchina",
            executables: vec!["macchina".into()],
            arguments: vec!["--config".into(), macchina_config.into()],
            version_arguments: &["--version"],
        },
        ToolSpec {
            name: "neofetch",
            executables: vec!["neofetch".into()],
            arguments: vec!["--config".into(), "none".into()],
            version_arguments: &["--version"],
        },
        ToolSpec {
            name: "neowofetch",
            executables: vec!["neowofetch".into()],
            arguments: vec!["--config".into(), "none".into()],
            version_arguments: &["--version"],
        },
        ToolSpec {
            name: "screenfetch",
            executables: vec!["screenfetch".into(), "screenfetch-dev".into()],
            arguments: Vec::new(),
            version_arguments: &["--version"],
        },
        ToolSpec {
            name: "nerdfetch",
            executables: vec!["nerdfetch".into()],
            arguments: Vec::new(),
            version_arguments: &["-v"],
        },
    ]
}

fn preflight_tool(spec: &ToolSpec) -> Result<AvailableTool, String> {
    let mut failures = Vec::new();
    let mut found_executable = false;

    for executable in &spec.executables {
        let executable = executable.as_str();
        let output = match run_captured(executable, &spec.arguments) {
            Ok(output) => {
                found_executable = true;
                output
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => {
                found_executable = true;
                failures.push(format!("{executable} could not start: {error}"));
                continue;
            }
        };

        if !output.status.success() {
            failures.push(format!(
                "{executable} preflight exited with {}{}",
                output.status,
                output_detail(&output)
            ));
            continue;
        }

        let version =
            capture_version(executable, spec.version_arguments).unwrap_or_else(|| "unknown".into());
        return Ok(AvailableTool {
            name: spec.name,
            version,
            arguments: spec.arguments.clone(),
            command: command_line(executable, &spec.arguments),
        });
    }

    if found_executable {
        Err(failures.join("; "))
    } else {
        Err(format!(
            "executable not found (tried {})",
            spec.executables.join(", ")
        ))
    }
}

fn run_captured(executable: &str, arguments: &[String]) -> io::Result<Output> {
    Command::new(executable)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
}

fn capture_version(executable: &str, arguments: &[&str]) -> Option<String> {
    let output = Command::new(executable)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .ok()?;

    first_nonempty_line(&output.stdout)
        .or_else(|| first_nonempty_line(&output.stderr))
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
}

fn first_nonempty_line(bytes: &[u8]) -> Option<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToOwned::to_owned)
}

fn output_detail(output: &Output) -> String {
    first_nonempty_line(&output.stderr)
        .or_else(|| first_nonempty_line(&output.stdout))
        .map(|line| format!(": {line}"))
        .unwrap_or_default()
}

fn command_line(executable: &str, arguments: &[String]) -> String {
    std::iter::once(executable)
        .chain(arguments.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ")
}

fn required_hyperfine_version() -> Result<String, String> {
    capture_version("hyperfine", &["--version"]).ok_or_else(|| {
        "hyperfine is required but unavailable; install it from https://github.com/sharkdp/hyperfine"
            .into()
    })
}

fn run_hyperfine(
    tools: &[AvailableTool],
    json_output: &Path,
    markdown_output: &Path,
) -> Result<(), String> {
    let mut command = Command::new("hyperfine");
    command
        .arg("--warmup")
        .arg(WARMUP_RUNS.to_string())
        .arg("--min-runs")
        .arg(MINIMUM_RUNS.to_string())
        .arg("--shell=none")
        .arg("--output=pipe")
        .arg("--export-json")
        .arg(json_output)
        .arg("--export-markdown")
        .arg(markdown_output);

    for tool in tools {
        command
            .arg("--command-name")
            .arg(tool.name)
            .arg(&tool.command);
    }

    let status = command
        .status()
        .map_err(|error| format!("could not start hyperfine: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("hyperfine failed with {status}"))
    }
}

fn collect_host_context(purr_executable: &str) -> BTreeMap<String, String> {
    let mut host = BTreeMap::new();
    let arguments = vec!["--no-config".into(), "--json".into()];
    let Ok(output) = run_captured(purr_executable, &arguments) else {
        return host;
    };
    let Ok(document) = serde_json::from_slice::<Value>(&output.stdout) else {
        return host;
    };

    if let Some(distro) = document.get("distro").and_then(Value::as_str) {
        host.insert("distro".into(), distro.into());
    }

    let Some(probes) = document.get("probes").and_then(Value::as_array) else {
        return host;
    };
    for probe in probes {
        let Some(identifier) = probe.get("id").and_then(Value::as_str) else {
            continue;
        };
        if !matches!(identifier, "model" | "kernel" | "cpu" | "memory") {
            continue;
        }
        if let Some(value) = probe.get("value").and_then(Value::as_str) {
            host.insert(identifier.into(), value.into());
        }
    }
    host
}

fn read_json(path: &Path) -> Result<Value, String> {
    let raw =
        fs::read(path).map_err(|error| format!("could not read {}: {error}", path.display()))?;
    serde_json::from_slice::<Value>(&raw)
        .map_err(|error| format!("could not parse {}: {error}", path.display()))
}

/// Reads hyperfine 1.x (flat `mean`/`median` in seconds) and 2.x (`summary.<metric>`) exports.
fn hyperfine_summaries(document: &Value) -> Vec<ToolSummary> {
    document
        .get("results")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|result| {
            let name = result
                .get("name")
                .or_else(|| result.get("command"))?
                .as_str()?;
            let wall = result.pointer("/summary/time_wall_clock").unwrap_or(result);
            let milliseconds = |key: &str| Some(wall.get(key)?.as_f64()? * 1000.0);
            Some(ToolSummary {
                name: name.to_owned(),
                median_ms: milliseconds("median")?,
                mean_ms: milliseconds("mean")?,
                stddev_ms: milliseconds("stddev").unwrap_or(0.0),
                min_ms: milliseconds("min")?,
                max_ms: milliseconds("max")?,
                peak_rss_median_bytes: result
                    .pointer("/summary/memory_peak_resident/median")
                    .and_then(Value::as_f64)
                    .map(|bytes| bytes as u64),
            })
        })
        .collect()
}

fn evaluate_budget(summaries: &[ToolSummary]) -> Result<BudgetVerdict, String> {
    let median_ms = |name: &str| {
        summaries
            .iter()
            .find(|summary| summary.name == name)
            .map(|summary| summary.median_ms)
    };
    let purr_median_ms =
        median_ms("purr").ok_or_else(|| "hyperfine results contain no purr entry".to_string())?;
    let fastfetch_median_ms = median_ms("fastfetch");
    let beats_fastfetch = match fastfetch_median_ms {
        Some(fastfetch) if purr_median_ms < fastfetch => Outcome::Pass,
        Some(_) => Outcome::Fail,
        None => Outcome::Skipped,
    };
    let within_soft_budget = if purr_median_ms <= SOFT_BUDGET_MS {
        Outcome::Pass
    } else {
        Outcome::Warn
    };
    let neofetch = ["neofetch", "neowofetch"].into_iter().find_map(|tool| {
        let median_ms = median_ms(tool)?;
        Some(NeofetchRatio {
            tool: tool.to_owned(),
            median_ms,
            speedup: median_ms / purr_median_ms,
        })
    });

    Ok(BudgetVerdict {
        purr_median_ms,
        fastfetch_median_ms,
        beats_fastfetch,
        soft_budget_ms: SOFT_BUDGET_MS,
        within_soft_budget,
        neofetch,
    })
}

fn print_verdict(verdict: &BudgetVerdict) {
    let colour = |outcome: Outcome| match outcome {
        Outcome::Pass => outcome.label().green(),
        Outcome::Warn | Outcome::Skipped => outcome.label().yellow(),
        Outcome::Info => outcome.label().cyan(),
        Outcome::Fail => outcome.label().red(),
    };
    println!("\nBudget verdict (report only; never fails the run):");
    for line in verdict_lines(verdict) {
        println!("  [{}] {}", colour(line.0), line.1);
    }
}

fn verdict_lines(verdict: &BudgetVerdict) -> Vec<(Outcome, String)> {
    let hard = match verdict.fastfetch_median_ms {
        Some(fastfetch) => format!(
            "HARD: purr median {:.1} ms vs fastfetch median {fastfetch:.1} ms",
            verdict.purr_median_ms
        ),
        None => "HARD: purr vs fastfetch (fastfetch unavailable)".to_string(),
    };
    let soft = format!(
        "SOFT: purr median {:.1} ms <= {:.0} ms",
        verdict.purr_median_ms, verdict.soft_budget_ms
    );
    let mut lines = vec![
        (verdict.beats_fastfetch, hard),
        (verdict.within_soft_budget, soft),
    ];
    match &verdict.neofetch {
        Some(ratio) => lines.push((
            Outcome::Info,
            format!(
                "purr is {:.0}x faster than {} ({:.1} ms)",
                ratio.speedup, ratio.tool, ratio.median_ms
            ),
        )),
        None => lines.push((
            Outcome::Skipped,
            "neofetch/neowofetch unavailable".to_string(),
        )),
    }
    lines
}

/// Nearest-rank percentile of an ascending, non-empty slice.
fn nearest_rank<T: Copy>(sorted: &[T], percent: usize) -> T {
    let rank = (percent * sorted.len()).div_ceil(100).max(1);
    sorted[rank - 1]
}

fn build_traced_purr(repository_root: &Path) -> Result<PathBuf, String> {
    let status = Command::new("cargo")
        .args([
            "build",
            "--release",
            "--locked",
            "--features",
            "profile",
            "--bin",
            "purr",
            "--target-dir",
            TRACE_TARGET_DIRECTORY,
        ])
        .status()
        .map_err(|error| format!("could not start cargo: {error}"))?;
    if !status.success() {
        return Err(format!("traced cargo build failed with {status}"));
    }
    Ok(repository_root.join(format!(
        "{TRACE_TARGET_DIRECTORY}/release/purr{}",
        env::consts::EXE_SUFFIX
    )))
}

/// Runs the traced build cold, once per process, and aggregates every span it records.
fn measure_spans(
    traced_purr: &Path,
    arguments: &[String],
    trace_directory: &Path,
) -> Result<Vec<SpanTiming>, String> {
    fs::create_dir_all(trace_directory)
        .map_err(|error| format!("could not create {}: {error}", trace_directory.display()))?;
    let trace_file = trace_directory.join("purr-trace.json");
    let mut samples = BTreeMap::<String, Vec<f64>>::new();

    for _ in 0..TRACE_RUNS {
        remove_if_present(&trace_file)?;
        // purr writes purr-trace.json into its working directory; stdout is drained like hyperfine.
        let output = Command::new(traced_purr)
            .args(arguments)
            .current_dir(trace_directory)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|error| format!("could not start {}: {error}", traced_purr.display()))?;
        if !output.status.success() {
            return Err(format!(
                "traced purr exited with {}{}",
                output.status,
                output_detail(&output)
            ));
        }
        for (span, duration_ms) in parse_trace(&read_json(&trace_file)?)? {
            samples.entry(span).or_default().push(duration_ms);
        }
    }
    remove_if_present(&trace_file)?;

    Ok(summarize_spans(samples))
}

/// Pairs Chrome trace `B`/`E` events per thread into `(span label, duration in ms)`.
fn parse_trace(document: &Value) -> Result<Vec<(String, f64)>, String> {
    let events = document
        .as_array()
        .ok_or_else(|| "trace is not a JSON array".to_string())?;
    let mut open = BTreeMap::<u64, Vec<(String, f64)>>::new();
    let mut durations = Vec::new();

    for event in events {
        let (Some(phase), Some(thread), Some(timestamp)) = (
            event.get("ph").and_then(Value::as_str),
            event.get("tid").and_then(Value::as_u64),
            event.get("ts").and_then(Value::as_f64),
        ) else {
            continue;
        };
        match phase {
            "B" => open
                .entry(thread)
                .or_default()
                .push((span_label(event), timestamp)),
            "E" => {
                if let Some((label, start)) = open.get_mut(&thread).and_then(Vec::pop) {
                    durations.push((label, (timestamp - start) / 1000.0));
                }
            }
            _ => {}
        }
    }
    Ok(durations)
}

/// `probe: CPU`, `subprocess: sw_vers`, or the bare span name when it carries no fields.
fn span_label(event: &Value) -> String {
    let name = event.get("name").and_then(Value::as_str).unwrap_or("?");
    let fields = event
        .get("args")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter(|(key, _)| key.as_str() != "message")
        .filter_map(|(_, value)| value.as_str())
        .map(|value| value.trim_matches('"'))
        .collect::<Vec<_>>();
    if fields.is_empty() {
        name.to_owned()
    } else {
        format!("{name}: {}", fields.join(", "))
    }
}

/// Slowest median first, so the probes that bound a run lead the table.
fn summarize_spans(samples: BTreeMap<String, Vec<f64>>) -> Vec<SpanTiming> {
    let mut spans = samples
        .into_iter()
        .map(|(span, mut durations)| {
            durations.sort_by(f64::total_cmp);
            SpanTiming {
                span,
                median_ms: nearest_rank(&durations, 50),
                p95_ms: nearest_rank(&durations, 95),
                samples: durations.len(),
            }
        })
        .collect::<Vec<_>>();
    spans.sort_by(|left, right| right.median_ms.total_cmp(&left.median_ms));
    spans
}

fn enrich_json(
    mut document: Value,
    destination: &Path,
    hyperfine_version: &str,
    context: &BenchmarkContext,
    measurements: &Measurements,
) -> Result<(), String> {
    inject_json_context(&mut document, hyperfine_version, context, measurements)?;

    let rendered = serde_json::to_string_pretty(&document)
        .map_err(|error| format!("could not serialize benchmark results: {error}"))?;
    fs::write(destination, format!("{rendered}\n"))
        .map_err(|error| format!("could not write {}: {error}", destination.display()))
}

fn inject_json_context(
    document: &mut Value,
    hyperfine_version: &str,
    context: &BenchmarkContext,
    measurements: &Measurements,
) -> Result<(), String> {
    let object = document
        .as_object_mut()
        .ok_or_else(|| "hyperfine JSON output was not an object".to_string())?;

    // hyperfine 2 owns the top-level `schema_version`, so this file's own version has its own key.
    object.insert("bench_schema_version".into(), Value::from(2));
    object.insert(
        "hyperfine_version".into(),
        Value::from(hyperfine_version.to_owned()),
    );
    object.insert(
        "context".into(),
        serde_json::to_value(context)
            .map_err(|error| format!("could not serialize benchmark context: {error}"))?,
    );
    object.insert(
        "measurements".into(),
        serde_json::to_value(measurements)
            .map_err(|error| format!("could not serialize measurements: {error}"))?,
    );
    Ok(())
}

fn enrich_markdown(
    source: &Path,
    destination: &Path,
    hyperfine_version: &str,
    context: &BenchmarkContext,
    measurements: &Measurements,
) -> Result<(), String> {
    let results = fs::read_to_string(source)
        .map_err(|error| format!("could not read {}: {error}", source.display()))?;
    let rendered = render_markdown(&results, hyperfine_version, context, measurements);
    fs::write(destination, rendered)
        .map_err(|error| format!("could not write {}: {error}", destination.display()))
}

fn render_markdown(
    results: &str,
    hyperfine_version: &str,
    context: &BenchmarkContext,
    measurements: &Measurements,
) -> String {
    let mut markdown = format!(
        "# Competitive benchmark results\n\n\
         The wall-time results cover process completion after each command's stdout is fully \
         drained. They do not report time-to-first-paint. Span timings come from a separate \
         `profile`-feature build of purr.\n\n\
         ## Context\n\n\
         - Platform: `{os}/{architecture}` (`{family}`), {logical_cpus} logical CPUs\n\
         - Sampling: {warmups} warmups, at least {minimum} measured runs\n\
         - Output handling: `{output_mode}`\n\
         - Hyperfine: `{hyperfine}`\n",
        os = context.platform.os,
        architecture = context.platform.architecture,
        family = context.platform.family,
        logical_cpus = context.platform.logical_cpus,
        warmups = context.sampling.warmup_runs,
        minimum = context.sampling.minimum_runs,
        output_mode = context.sampling.output_mode,
        hyperfine = markdown_cell(hyperfine_version),
    );

    for (key, value) in &context.host {
        markdown.push_str(&format!(
            "- {}: `{}`\n",
            title_case(key),
            markdown_cell(value)
        ));
    }

    markdown.push_str("\n## Commands\n\n| Tool | Version | Command |\n|---|---|---|\n");
    for tool in &context.tools {
        markdown.push_str(&format!(
            "| {} | `{}` | `{}` |\n",
            tool.name,
            markdown_cell(&tool.version),
            markdown_cell(&tool.command)
        ));
    }

    if !context.skipped.is_empty() {
        markdown.push_str("\n## Skipped tools\n\n");
        for skipped in &context.skipped {
            markdown.push_str(&format!(
                "- **{}**: {}\n",
                skipped.name,
                markdown_cell(&skipped.reason)
            ));
        }
    }

    markdown.push_str("\n## Results\n\n");
    markdown.push_str(results.trim());
    markdown.push('\n');

    markdown.push_str("\n## Budget verdict\n\n");
    for (outcome, line) in verdict_lines(&measurements.verdict) {
        markdown.push_str(&format!("- **{}** {}\n", outcome.label(), line));
    }

    markdown.push_str(
        "\n## Summary\n\n\
         | Tool | Median (ms) | Mean ± σ (ms) | Min (ms) | Max (ms) | Median peak RSS (MiB) |\n\
         |---|---:|---:|---:|---:|---:|\n",
    );
    for tool in &measurements.tools {
        let rss = tool
            .peak_rss_median_bytes
            .map(|bytes| format!("{:.1}", mebibytes(bytes)))
            .unwrap_or_else(|| "n/a".into());
        markdown.push_str(&format!(
            "| {} | {:.1} | {:.1} ± {:.1} | {:.1} | {:.1} | {rss} |\n",
            markdown_cell(&tool.name),
            tool.median_ms,
            tool.mean_ms,
            tool.stddev_ms,
            tool.min_ms,
            tool.max_ms
        ));
    }
    markdown.push_str(&format!(
        "\npurr release binary: {} bytes ({:.2} MiB)\n",
        measurements.purr_binary_bytes,
        mebibytes(measurements.purr_binary_bytes)
    ));

    markdown.push_str(&format!(
        "\n## Cold span timings\n\n\
         {} cold runs of purr built with `--features profile`; spans overlap because probes run \
         in parallel.\n\n\
         | Span | Median (ms) | p95 (ms) | Samples |\n|---|---|---|---|\n",
        measurements.trace_runs
    ));
    if let Some(reason) = &measurements.span_error {
        markdown.push_str(&format!("| _skipped: {}_ | | | |\n", markdown_cell(reason)));
    }
    for span in &measurements.spans {
        markdown.push_str(&format!(
            "| {} | {:.2} | {:.2} | {} |\n",
            markdown_cell(&span.span),
            span.median_ms,
            span.p95_ms,
            span.samples
        ));
    }
    markdown
}

fn mebibytes(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

fn markdown_cell(value: &str) -> String {
    value
        .replace('|', "\\|")
        .replace('`', "\\`")
        .replace(['\r', '\n'], " ")
}

fn title_case(value: &str) -> String {
    if value == "cpu" {
        return "CPU".into();
    }

    let mut characters = value.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
        None => String::new(),
    }
}

fn purr_executable(executable_suffix: &str) -> String {
    format!("target/release/purr{executable_suffix}")
}

fn relative_path(repository_root: &Path, path: &Path) -> Result<String, String> {
    path.strip_prefix(repository_root)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .map_err(|error| {
            format!(
                "{} is not under {}: {error}",
                path.display(),
                repository_root.display()
            )
        })
}

fn remove_if_present(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("could not remove {}: {error}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> BenchmarkContext {
        BenchmarkContext {
            platform: PlatformContext {
                os: "linux",
                family: "unix",
                architecture: "x86_64",
                logical_cpus: 8,
            },
            host: BTreeMap::from([("cpu".into(), "Test CPU".into())]),
            sampling: SamplingContext {
                warmup_runs: WARMUP_RUNS,
                minimum_runs: MINIMUM_RUNS,
                output_mode: OUTPUT_MODE,
                timing_scope: "process completion after stdout is fully drained",
            },
            tools: vec![ToolContext {
                name: "purr",
                version: "purr 1.0.0".into(),
                command: "target/release/purr --no-config".into(),
            }],
            skipped: vec![SkippedTool {
                name: "neofetch",
                reason: "executable not found".into(),
            }],
        }
    }

    fn summary(name: &str, median_ms: f64) -> ToolSummary {
        ToolSummary {
            name: name.into(),
            median_ms,
            mean_ms: median_ms + 0.5,
            stddev_ms: 1.0,
            min_ms: median_ms - 1.0,
            max_ms: median_ms + 2.0,
            peak_rss_median_bytes: None,
        }
    }

    fn measurements() -> Measurements {
        let tools = vec![
            ToolSummary {
                peak_rss_median_bytes: Some(8 * 1024 * 1024),
                ..summary("purr", 12.0)
            },
            summary("fastfetch", 20.0),
            summary("neowofetch", 600.0),
        ];
        Measurements {
            purr_binary_bytes: 6_793_656,
            verdict: evaluate_budget(&tools).unwrap(),
            tools,
            trace_runs: TRACE_RUNS,
            spans: vec![SpanTiming {
                span: "probe: GPU".into(),
                median_ms: 4.5,
                p95_ms: 6.25,
                samples: 20,
            }],
            span_error: None,
        }
    }

    #[test]
    fn reads_hyperfine_2_summaries() {
        let document = serde_json::json!({"results": [
            {"command": "target/release/purr --stdout", "name": "purr", "summary": {
                "time_wall_clock": {"unit": "second", "mean": 0.006, "stddev": 0.002,
                    "median": 0.005, "min": 0.004, "max": 0.03},
                "memory_peak_resident": {"unit": "byte", "median": 7864320.0},
            }},
            {"name": "broken", "summary": {}},
        ]});

        let summaries = hyperfine_summaries(&document);
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].name, "purr");
        assert!((summaries[0].median_ms - 5.0).abs() < 1e-9);
        assert!((summaries[0].max_ms - 30.0).abs() < 1e-9);
        assert_eq!(summaries[0].peak_rss_median_bytes, Some(7_864_320));
    }

    #[test]
    fn reads_hyperfine_1_results_without_memory() {
        let document = serde_json::json!({"results": [
            {"command": "fastfetch", "mean": 0.0125, "stddev": 0.001, "median": 0.0125,
                "min": 0.01, "max": 0.02},
        ]});

        let summaries = hyperfine_summaries(&document);
        assert_eq!(summaries[0].name, "fastfetch");
        assert_eq!(summaries[0].median_ms, 12.5);
        assert_eq!(summaries[0].peak_rss_median_bytes, None);
    }

    #[test]
    fn passes_budgets_when_purr_beats_fastfetch_within_soft_budget() {
        let verdict = measurements().verdict;

        assert_eq!(verdict.beats_fastfetch, Outcome::Pass);
        assert_eq!(verdict.within_soft_budget, Outcome::Pass);
        let neofetch = verdict.neofetch.unwrap();
        assert_eq!(neofetch.tool, "neowofetch");
        assert_eq!(neofetch.speedup, 50.0);
    }

    #[test]
    fn fails_hard_budget_and_warns_soft_budget_when_slower() {
        let verdict =
            evaluate_budget(&[summary("purr", 30.0), summary("fastfetch", 30.0)]).unwrap();

        assert_eq!(verdict.beats_fastfetch, Outcome::Fail);
        assert_eq!(verdict.within_soft_budget, Outcome::Warn);
        assert_eq!(verdict.neofetch, None);
    }

    #[test]
    fn skips_hard_budget_without_fastfetch_and_prefers_neofetch() {
        let verdict = evaluate_budget(&[
            summary("purr", 10.0),
            summary("neofetch", 500.0),
            summary("neowofetch", 900.0),
        ])
        .unwrap();

        assert_eq!(verdict.beats_fastfetch, Outcome::Skipped);
        assert_eq!(verdict.neofetch.unwrap().tool, "neofetch");
        assert!(evaluate_budget(&[summary("fastfetch", 1.0)]).is_err());
    }

    #[test]
    fn takes_nearest_rank_percentiles() {
        let values = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        assert_eq!(nearest_rank(&values, 50), 5);
        assert_eq!(nearest_rank(&values, 95), 10);
        assert_eq!(nearest_rank(&[7], 50), 7);
    }

    #[test]
    fn pairs_trace_spans_per_thread() {
        let trace = serde_json::json!([
            {"ph": "M", "pid": 1, "tid": 0, "name": "thread_name", "args": {"name": "main"}},
            {"ph": "B", "pid": 1, "tid": 1, "ts": 100.0, "name": "probe", "args": {"name": "GPU"}},
            {"ph": "B", "pid": 1, "tid": 2, "ts": 150.0, "name": "init_readout", "args": {"kind": "\"general\""}},
            {"ph": "B", "pid": 1, "tid": 1, "ts": 200.0, "name": "gpu_udev"},
            {"ph": "i", "pid": 1, "tid": 1, "ts": 250.0, "name": "event", "args": {"message": "hi"}},
            {"ph": "E", "pid": 1, "tid": 1, "ts": 1200.0, "name": "gpu_udev"},
            {"ph": "E", "pid": 1, "tid": 2, "ts": 650.0, "name": "init_readout", "args": {"kind": "\"general\""}},
            {"ph": "E", "pid": 1, "tid": 1, "ts": 2100.0, "name": "probe", "args": {"name": "GPU"}},
        ]);

        assert_eq!(
            parse_trace(&trace).unwrap(),
            vec![
                ("gpu_udev".to_string(), 1.0),
                ("init_readout: general".to_string(), 0.5),
                ("probe: GPU".to_string(), 2.0),
            ]
        );
        assert!(parse_trace(&serde_json::json!({})).is_err());
    }

    #[test]
    fn summarizes_spans_slowest_first() {
        let spans = summarize_spans(BTreeMap::from([
            ("probe: CPU".to_string(), vec![1.0, 3.0, 2.0]),
            ("probe: GPU".to_string(), vec![9.0]),
        ]));

        assert_eq!(spans[0].span, "probe: GPU");
        assert_eq!(
            spans[1],
            SpanTiming {
                span: "probe: CPU".into(),
                median_ms: 2.0,
                p95_ms: 3.0,
                samples: 3,
            }
        );
    }

    #[test]
    fn constructs_platform_specific_purr_path() {
        assert_eq!(purr_executable(""), "target/release/purr");
        assert_eq!(purr_executable(".exe"), "target/release/purr.exe");
    }

    #[test]
    fn extracts_the_first_nonempty_version_line() {
        assert_eq!(
            first_nonempty_line(b"\n\nhyperfine 1.20.0\nmore"),
            Some("hyperfine 1.20.0".into())
        );
        assert_eq!(first_nonempty_line(b"\n"), None);
    }

    #[test]
    fn renders_reproducibility_context_in_markdown() {
        let markdown = render_markdown(
            "| Command | Mean |\n|---|---|",
            "hyperfine 1.20.0",
            &context(),
            &measurements(),
        );

        assert!(markdown.contains("process completion"));
        assert!(
            markdown.contains("- **PASS** HARD: purr median 12.0 ms vs fastfetch median 20.0 ms")
        );
        assert!(markdown.contains("- **INFO** purr is 50x faster than neowofetch (600.0 ms)"));
        assert!(markdown.contains("6793656 bytes (6.48 MiB)"));
        assert!(markdown.contains("| purr | 12.0 | 12.5 ± 1.0 | 11.0 | 14.0 | 8.0 |"));
        assert!(markdown.contains("| fastfetch | 20.0 | 20.5 ± 1.0 | 19.0 | 22.0 | n/a |"));
        assert!(markdown.contains("| probe: GPU | 4.50 | 6.25 | 20 |"));
        assert!(markdown.contains("5 warmups"));
        assert!(markdown.contains("Test CPU"));
        assert!(markdown.contains("target/release/purr --no-config"));
        assert!(markdown.contains("**neofetch**: executable not found"));
        assert!(markdown.contains("| Command | Mean |"));
    }

    #[test]
    fn renders_unavailable_competitors_and_skipped_spans() {
        let tools = vec![summary("purr", 30.0)];
        let measurements = Measurements {
            purr_binary_bytes: 1,
            verdict: evaluate_budget(&tools).unwrap(),
            tools,
            trace_runs: TRACE_RUNS,
            spans: Vec::new(),
            span_error: Some("traced cargo build failed".into()),
        };

        let markdown = render_markdown("", "hyperfine 2.0.0", &context(), &measurements);

        assert!(markdown.contains("- **SKIPPED** HARD: purr vs fastfetch (fastfetch unavailable)"));
        assert!(markdown.contains("- **WARN** SOFT: purr median 30.0 ms <= 25 ms"));
        assert!(markdown.contains("- **SKIPPED** neofetch/neowofetch unavailable"));
        assert!(markdown.contains("| _skipped: traced cargo build failed_ | | | |"));
    }

    #[test]
    fn escapes_markdown_table_content() {
        assert_eq!(markdown_cell("one|two\nthree`"), "one\\|two three\\`");
    }

    #[test]
    fn enriches_hyperfine_json_without_replacing_results() {
        let mut document = serde_json::json!({
            "results": [{"command": "purr", "mean": 0.02}]
        });

        inject_json_context(
            &mut document,
            "hyperfine 1.20.0",
            &context(),
            &measurements(),
        )
        .unwrap();

        assert_eq!(document["bench_schema_version"], 2);
        assert_eq!(
            document["measurements"]["verdict"]["beats_fastfetch"],
            "pass"
        );
        assert_eq!(document["measurements"]["spans"][0]["span"], "probe: GPU");
        assert_eq!(document["hyperfine_version"], "hyperfine 1.20.0");
        assert_eq!(document["context"]["sampling"]["warmup_runs"], 5);
        assert_eq!(document["results"][0]["command"], "purr");
    }
}
