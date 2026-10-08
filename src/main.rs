use anyhow::{Context, Result, ensure};
use clap::{Parser, Subcommand};
use dfa_minimization_partition_trace::{
    dfa::Dfa,
    hopcroft, moore,
    partition::{self, Metrics, Refinement},
    render,
    trace::{Cause, Trace},
    verify,
};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

#[derive(Parser)]
#[command(
    version,
    about = "Explain and compare Moore and Hopcroft DFA minimization"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Minimize a complete JSON DFA with both algorithms and verify their results.
    Compare {
        input: PathBuf,
        /// Output directory; must be absent or empty to protect existing artifacts.
        #[arg(long, default_value = "output")]
        out: PathBuf,
        /// Exhaustively compare every word up to this length, including the empty word.
        #[arg(long, default_value_t = 8)]
        max_word_length: u32,
        /// Refuse exhaustive enumeration above this number of words.
        #[arg(long, default_value_t = 1_000_000)]
        max_words: u64,
        /// Untimed trace recording runs separately from these measured refinement runs.
        #[arg(long, default_value_t = 5, value_parser = clap::value_parser!(u32).range(1..=100))]
        repetitions: u32,
        /// Skip potentially large split traces (verification and graphs still run).
        #[arg(long)]
        no_trace: bool,
    },
}

#[derive(Serialize)]
struct Timing {
    repetitions: u32,
    median_us: f64,
    min_us: f64,
    samples_us: Vec<f64>,
}

#[derive(Serialize)]
struct AlgorithmReport {
    minimized_states: usize,
    metrics: Metrics,
    refinement_timing: Timing,
}

#[derive(Serialize)]
struct Verification {
    original_vs_moore: verify::Equivalence,
    original_vs_hopcroft: verify::Equivalence,
    moore_vs_hopcroft: verify::Equivalence,
    canonical_machines_equal: bool,
    exhaustive: verify::ExhaustiveCheck,
}

#[derive(Serialize)]
struct Report {
    schema_version: u32,
    input: String,
    original_states: usize,
    reachable_states: usize,
    unreachable_states: Vec<String>,
    alphabet: Vec<String>,
    moore: AlgorithmReport,
    hopcroft: AlgorithmReport,
    verification: Verification,
    preprocessing_us: f64,
    quotient_construction_us: f64,
    exact_verification_us: f64,
    exhaustive_verification_us: f64,
    trace_recording_us: f64,
    graph_rendering_us: f64,
    traces_enabled: bool,
}

fn micros(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1_000_000.0
}

fn benchmark(
    dfa: &Dfa,
    repetitions: u32,
    algorithm: fn(&Dfa, bool) -> Refinement,
) -> (Refinement, Timing) {
    // One warm-up reduces first-use effects, but these timings are still descriptive.
    std::hint::black_box(algorithm(std::hint::black_box(dfa), false));
    let mut samples = Vec::new();
    let mut result = None;
    for _ in 0..repetitions {
        let start = Instant::now();
        let refinement = algorithm(std::hint::black_box(dfa), false);
        samples.push(micros(start));
        result = Some(std::hint::black_box(refinement));
    }
    let mut sorted = samples.clone();
    sorted.sort_by(f64::total_cmp);
    let mid = sorted.len() / 2;
    let median = if sorted.len() % 2 == 0 {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    };
    (
        result.unwrap(),
        Timing {
            repetitions,
            median_us: median,
            min_us: sorted[0],
            samples_us: samples,
        },
    )
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    fs::write(path, bytes).with_context(|| format!("cannot write {}", path.display()))
}

fn trace_text(trace: &Trace) -> String {
    use std::fmt::Write;
    let mut text = format!("{} partition trace\n", trace.algorithm);
    for block in &trace.initial_partition {
        writeln!(text, "Initial B{}: {:?}", block.id, block.states).unwrap();
    }
    for event in &trace.events {
        writeln!(
            text,
            "\nStep {}, symbol {:?}: B{} {:?}",
            event.step, event.symbol, event.source.id, event.source.states
        )
        .unwrap();
        for (index, child) in event.children.iter().enumerate() {
            let reason = match &event.cause {
                Cause::DestinationBlocks { destination_blocks } => format!(
                    "enters B{} from the start-of-round partition",
                    destination_blocks[index]
                ),
                Cause::Splitter { splitter } => format!(
                    "{} the step's splitter {:?}",
                    if index == 0 {
                        "enters"
                    } else {
                        "does not enter"
                    },
                    splitter.states
                ),
            };
            writeln!(text, "  -> B{} {:?}: {reason}", child.id, child.states).unwrap();
        }
    }
    if trace.events.is_empty() {
        text.push_str("\nNo splits: the initial partition is stable.\n");
    }
    text
}

fn main() -> Result<()> {
    let Cli {
        command:
            Command::Compare {
                input,
                out,
                max_word_length,
                max_words,
                repetitions,
                no_trace,
            },
    } = Cli::parse();
    if out.exists() {
        ensure!(
            out.is_dir() && fs::read_dir(&out)?.next().is_none(),
            "output directory {} must be empty; choose a fresh --out directory",
            out.display()
        );
    }
    let start = Instant::now();
    let text =
        fs::read_to_string(&input).with_context(|| format!("cannot read {}", input.display()))?;
    let original = Dfa::from_json(&text).context("invalid DFA")?;
    let (reachable, unreachable_states) = original.reachable();
    let preprocessing_us = micros(start);
    verify::word_count(original.alphabet.len(), max_word_length, max_words)?;

    let (moore, moore_timing) = benchmark(&reachable, repetitions, moore::refine);
    let (hopcroft, hopcroft_timing) = benchmark(&reachable, repetitions, hopcroft::refine);
    let start = Instant::now();
    let moore_min = partition::quotient(&reachable, &moore.blocks);
    let hopcroft_min = partition::quotient(&reachable, &hopcroft.blocks);
    let quotient_construction_us = micros(start);
    let start = Instant::now();
    let original_vs_moore = verify::equivalent(&original, &moore_min.dfa)?;
    let original_vs_hopcroft = verify::equivalent(&original, &hopcroft_min.dfa)?;
    let moore_vs_hopcroft = verify::equivalent(&moore_min.dfa, &hopcroft_min.dfa)?;
    let canonical_machines_equal = verify::structurally_equal(&moore_min.dfa, &hopcroft_min.dfa);
    let exact_verification_us = micros(start);
    let start = Instant::now();
    let exhaustive = verify::exhaustive(
        &[&original, &moore_min.dfa, &hopcroft_min.dfa],
        max_word_length,
        max_words,
    )?;
    let exhaustive_verification_us = micros(start);
    let verification = Verification {
        original_vs_moore,
        original_vs_hopcroft,
        moore_vs_hopcroft,
        canonical_machines_equal,
        exhaustive,
    };
    let verified = verification.original_vs_moore.equivalent
        && verification.original_vs_hopcroft.equivalent
        && verification.moore_vs_hopcroft.equivalent
        && verification.canonical_machines_equal
        && verification.exhaustive.equivalent;

    let start = Instant::now();
    let traces = if no_trace {
        vec![]
    } else {
        vec![
            ("moore", moore::refine(&reachable, true).trace.unwrap()),
            (
                "hopcroft",
                hopcroft::refine(&reachable, true).trace.unwrap(),
            ),
        ]
    };
    let trace_recording_us = micros(start);
    let start = Instant::now();
    let graphs = [
        ("original", &original, "Original DFA", None),
        ("reachable", &reachable, "Reachable DFA", None),
        (
            "minimized",
            &moore_min.dfa,
            "Minimized DFA",
            Some(moore_min.members.as_slice()),
        ),
    ]
    .map(|(name, dfa, title, members)| {
        (
            name,
            render::svg(dfa, title, members),
            render::dot(dfa, members),
        )
    });
    let graph_rendering_us = micros(start);
    let report = Report {
        schema_version: 1,
        input: input.display().to_string(),
        original_states: original.names.len(),
        reachable_states: reachable.names.len(),
        unreachable_states,
        alphabet: original.alphabet.clone(),
        moore: AlgorithmReport {
            minimized_states: moore_min.dfa.names.len(),
            metrics: moore.metrics,
            refinement_timing: moore_timing,
        },
        hopcroft: AlgorithmReport {
            minimized_states: hopcroft_min.dfa.names.len(),
            metrics: hopcroft.metrics,
            refinement_timing: hopcroft_timing,
        },
        verification,
        preprocessing_us,
        quotient_construction_us,
        exact_verification_us,
        exhaustive_verification_us,
        trace_recording_us,
        graph_rendering_us,
        traces_enabled: !no_trace,
    };

    fs::create_dir_all(&out).with_context(|| format!("cannot create {}", out.display()))?;
    write_json(&out.join("moore.min.json"), &moore_min.dfa.to_json())?;
    write_json(&out.join("hopcroft.min.json"), &hopcroft_min.dfa.to_json())?;
    write_json(&out.join("members.json"), &moore_min.members)?;
    for (name, trace) in traces {
        write_json(&out.join(format!("{name}.trace.json")), &trace)?;
        fs::write(out.join(format!("{name}.trace.txt")), trace_text(&trace))?;
    }
    for (name, svg, dot) in graphs {
        fs::write(out.join(format!("{name}.svg")), svg)?;
        fs::write(out.join(format!("{name}.dot")), dot)?;
    }
    write_json(&out.join("report.json"), &report)?;
    ensure!(
        verified,
        "verification FAILED; see {} for counterexamples",
        out.join("report.json").display()
    );
    println!(
        "States: {} original -> {} reachable -> {} minimized",
        report.original_states, report.reachable_states, report.moore.minimized_states
    );
    println!(
        "Moore: {} rounds, {} splits, {} transition examinations, {:.3} us median",
        report.moore.metrics.moore_rounds.unwrap(),
        report.moore.metrics.partition_splits,
        report.moore.metrics.transition_examinations,
        report.moore.refinement_timing.median_us
    );
    println!(
        "Hopcroft: {} worklist steps, {} splits, {} transition examinations, {:.3} us median",
        report.hopcroft.metrics.hopcroft_steps.unwrap(),
        report.hopcroft.metrics.partition_splits,
        report.hopcroft.metrics.transition_examinations,
        report.hopcroft.refinement_timing.median_us
    );
    println!(
        "Verification: exact equivalence PASS; canonical comparison PASS; {} words through length {} PASS",
        report.verification.exhaustive.words_checked, max_word_length
    );
    println!("Artifacts: {}", out.display());
    Ok(())
}
