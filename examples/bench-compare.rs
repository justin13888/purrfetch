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
    command: String,
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
        arguments: vec!["--no-config".into()],
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
    enrich_json(temporary_json, &json_output, &hyperfine_version, &context)?;
    enrich_markdown(
        temporary_markdown,
        &markdown_output,
        &hyperfine_version,
        &context,
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
            arguments: vec!["--config".into(), "none".into()],
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

fn enrich_json(
    source: &Path,
    destination: &Path,
    hyperfine_version: &str,
    context: &BenchmarkContext,
) -> Result<(), String> {
    let raw = fs::read(source)
        .map_err(|error| format!("could not read {}: {error}", source.display()))?;
    let mut document = serde_json::from_slice::<Value>(&raw)
        .map_err(|error| format!("could not parse {}: {error}", source.display()))?;
    inject_json_context(&mut document, hyperfine_version, context)?;

    let rendered = serde_json::to_string_pretty(&document)
        .map_err(|error| format!("could not serialize benchmark results: {error}"))?;
    fs::write(destination, format!("{rendered}\n"))
        .map_err(|error| format!("could not write {}: {error}", destination.display()))
}

fn inject_json_context(
    document: &mut Value,
    hyperfine_version: &str,
    context: &BenchmarkContext,
) -> Result<(), String> {
    let object = document
        .as_object_mut()
        .ok_or_else(|| "hyperfine JSON output was not an object".to_string())?;

    object.insert("schema_version".into(), Value::from(1));
    object.insert(
        "hyperfine_version".into(),
        Value::from(hyperfine_version.to_owned()),
    );
    object.insert(
        "context".into(),
        serde_json::to_value(context)
            .map_err(|error| format!("could not serialize benchmark context: {error}"))?,
    );
    Ok(())
}

fn enrich_markdown(
    source: &Path,
    destination: &Path,
    hyperfine_version: &str,
    context: &BenchmarkContext,
) -> Result<(), String> {
    let results = fs::read_to_string(source)
        .map_err(|error| format!("could not read {}: {error}", source.display()))?;
    let rendered = render_markdown(&results, hyperfine_version, context);
    fs::write(destination, rendered)
        .map_err(|error| format!("could not write {}: {error}", destination.display()))
}

fn render_markdown(results: &str, hyperfine_version: &str, context: &BenchmarkContext) -> String {
    let mut markdown = format!(
        "# Competitive benchmark results\n\n\
         These measurements cover process completion after each command's stdout is fully drained. \
         They do not report time-to-first-paint or internal probe timings.\n\n\
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
    markdown
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
        );

        assert!(markdown.contains("process completion"));
        assert!(markdown.contains("5 warmups"));
        assert!(markdown.contains("Test CPU"));
        assert!(markdown.contains("target/release/purr --no-config"));
        assert!(markdown.contains("**neofetch**: executable not found"));
        assert!(markdown.contains("| Command | Mean |"));
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

        inject_json_context(&mut document, "hyperfine 1.20.0", &context()).unwrap();

        assert_eq!(document["schema_version"], 1);
        assert_eq!(document["hyperfine_version"], "hyperfine 1.20.0");
        assert_eq!(document["context"]["sampling"]["warmup_runs"], 5);
        assert_eq!(document["results"][0]["command"], "purr");
    }
}
