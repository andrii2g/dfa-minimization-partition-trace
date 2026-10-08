# DFA minimization partition trace

A Rust library and CLI that compare Moore-style partition refinement with Hopcroft's minimization algorithm. See which states merge, why other states split, and how much work each algorithm performs.

The CLI validates a complete deterministic automaton, removes unreachable states, minimizes it twice, proves language equivalence, compares short words, and exports graphs and explanatory traces.

## Quick start

Install a recent stable Rust toolchain with Cargo (Rust 2024 edition support is required). No Graphviz installation is needed.

```sh
cargo run --release -- compare examples/redundant.json --out output/demo
```

The example recognizes words ending in `ab`. It contains duplicate states and one unreachable state:

```text
States: 7 original -> 6 reachable -> 3 minimized
Moore: 2 rounds, 1 splits, 24 transition examinations, ... us median
Hopcroft: 4 worklist steps, 1 splits, 20 transition examinations, ... us median
Verification: exact equivalence PASS; canonical comparison PASS; 511 words through length 8 PASS
Artifacts: output/demo
```

Open `output/demo/original.svg` and `output/demo/minimized.svg` in a browser. Accepting states have double circles; the start state has an incoming arrow. Minimized nodes show their original members, with full names in hover tooltips. The built-in circular layout targets small teaching examples. DOT exports support external layouts:

```sh
dot -Tsvg output/demo/original.dot -o output/graphviz-original.svg
```

The output directory must be absent or empty. Choose a fresh directory for another run; existing files are never intentionally overwritten.

## CLI options

```sh
cargo run --release -- compare examples/chain.json --max-word-length 10 --repetitions 20 --out output/chain
cargo run --release -- compare examples/redundant.json --no-trace --out output/no-trace
cargo run -- compare --help
```

| Option | Default | Meaning |
| --- | --- | --- |
| `--out` | `output` | Empty/new artifact directory |
| `--max-word-length` | `8` | Exhaustive word length, including length zero |
| `--max-words` | `1000000` | Maximum allowed exhaustive search size |
| `--repetitions` | `5` | Measured refinement runs, from 1 to 100 |
| `--no-trace` | off | Skip JSON/text split traces and their memory cost |

For alphabet size `k`, enumeration visits `1 + k + ... + k^L` words. Excessive or overflowing requests are rejected before minimization. Exact equivalence always runs, including with `--max-word-length 0` or `--no-trace`.

## Input JSON

```json
{
  "alphabet": ["a", "b"],
  "states": ["s", "f1", "f2"],
  "start": "s",
  "accepting": ["f1", "f2"],
  "transitions": {
    "s":  {"a": "f1", "b": "f2"},
    "f1": {"a": "f1", "b": "f2"},
    "f2": {"a": "f1", "b": "f2"}
  }
}
```

- At least one state; names must be unique, nonempty strings.
- Up to 16 distinct symbols, each exactly one Unicode scalar value. Symbols are sorted internally for reproducible traversal.
- The empty alphabet is supported; each state still needs an empty transition row.
- Start and accepting states must exist. Duplicate accepting entries are rejected.
- Every state needs exactly one transition for every symbol. Unknown states/symbols, missing transitions, duplicate JSON transition keys, and unknown top-level fields are rejected.
- The automaton must already be complete. No implicit sink state is inserted. Reachable rejecting sink states are retained when needed to preserve completeness.

## Outputs

| File | Contents |
| --- | --- |
| `original.svg`, `reachable.svg`, `minimized.svg` | State-transition graphs |
| `original.dot`, `reachable.dot`, `minimized.dot` | Graphviz-compatible sources |
| `moore.min.json`, `hopcroft.min.json` | Reloadable complete DFAs, canonically named `M0`, `M1`, … |
| `members.json` | Original names grouped by canonical minimized-state index |
| `moore.trace.json`, `hopcroft.trace.json` | Initial partition and ordered split events |
| `moore.trace.txt`, `hopcroft.trace.txt` | Human-readable explanations |
| `report.json` | State counts, removed states, metrics, timings, and verification |

Both minimized machines are numbered by BFS from the start state, traversing sorted symbols. Their JSON structures therefore agree even when the algorithms discover blocks in different orders. Trace block IDs are local to their algorithm and are **not** minimized-state IDs.

## How refinement works

Both algorithms begin with accepting and nonaccepting states in separate nonempty blocks.

**Moore** freezes the partition at the start of each round. Within each block it groups states by the blocks reached on each symbol. Symbol-by-symbol subdivisions produce the same partition as grouping full destination signatures, while giving each trace event a precise cause. Dense destination buckets make a round linear in the transition count; worst-case time is `O(k n²)` for nonempty alphabets.

**Hopcroft** processes `(splitter block, symbol)` work items. A reverse-transition index finds states entering the splitter. Only affected blocks are visited. A position index permits constant-time removal of individual states; the unaffected complement is not copied during untraced refinement. Pending splitters are replaced by both children; otherwise only the smaller child is scheduled. The untraced bound is `O(k n log n)` with `O(kn)` storage, for nonempty alphabets.

Here `n` counts reachable states and `k` is the alphabet size. Trace events contain state-name snapshots and can require substantially more time and space. They are excluded from refinement timings.

Background: [Hopcroft's original paper](https://www.cs.cmu.edu/~cdm/resources/Hopcroft1971.pdf) and [Minimization of Automata](https://arxiv.org/abs/1010.5318).

## Reading a trace

```text
Step 1, symbol "b": B0 ["q0", "q0copy", "q1", "q1copy"]
  -> B0 ["q0", "q0copy"]: enters B0 from the start-of-round partition
  -> B2 ["q1", "q1copy"]: enters B1 from the start-of-round partition
```

Each JSON event includes `step`, `symbol`, `source`, `children`, and `cause`. A split retains the parent ID for one child and allocates fresh IDs to the others. Replaying events over `initial_partition` reconstructs the final partition.

- Moore `step` is a round number. `destination_blocks[i]` explains child `i` using the frozen start-of-round partition, even after other events in that round.
- Hopcroft `step` counts popped worklist entries; steps with no split have no event. The cause stores the splitter's membership at the beginning of that step. Child zero enters the splitter; child one does not.
- The initial accepting/nonaccepting separation is not counted as a split.

## Verification and metrics

Exact verification explores the reachable product of two machines. If acceptance differs, BFS reconstructs a shortest counterexample as a sequence of symbols. An empty sequence means the empty word; `null` means no counterexample. Checks cover original versus each result, and the results against each other. A failed check produces a nonzero exit status and diagnostics in `report.json`.

Bounded exhaustive verification independently enumerates all words through the requested length, simulating all three machines incrementally. This is a regression check, **not** a proof for longer words. Canonical structural comparison additionally checks that both algorithms produce the same minimized machine.

| Metric | Definition |
| --- | --- |
| Original/reachable/minimized states | Before pruning, after pruning, and final quotient size |
| Moore rounds | Full passes, including the final stable pass |
| Hopcroft steps | Worklist entries processed; zero for a one-block initial partition |
| Partition splits | Increase in block count: an `r`-way split counts as `r - 1` |
| Transition examinations | Moore: transition entries inspected; Hopcroft: reverse-index construction edges plus predecessor edges visited |
| Refinement timing | Median, minimum, and samples in microseconds, after one warm-up |

Timed runs disable tracing and include each algorithm's data-structure setup, including Hopcroft's reverse index. They exclude input parsing, reachability, quotient construction, verification, graph rendering, and file writes. The report separately times preprocessing (input read, parsing, validation, reachability), both quotients together, exact checks, exhaustive checks, both traces together, and all graph rendering. Artifact serialization/writes are not timed. Tiny-example runtimes are noisy and do not establish asymptotic performance.

## Examples and development

| Example | Purpose | Original → reachable → minimized |
| --- | --- | --- |
| `redundant.json` | Duplicate states, unreachable state, words ending in `ab` | 7 → 6 → 3 |
| `chain.json` | Distinctions propagate backward; contains seven consecutive `a` symbols | 8 → 8 → 8 |
| `already-minimal.json` | Even number of `a` symbols | 2 → 2 → 2 |
| `all-accepting.json` | Universal language | 2 → 2 → 1 |
| `all-rejecting.json` | Empty language | 2 → 2 → 1 |
| `empty-alphabet.json` | Empty word and unreachable-state edge case | 2 → 1 → 1 |

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

Tests cover all 5,898 complete binary DFAs with one to three states and a fixed start, plus 1,800 seeded larger automata. They check equivalence, canonical output, trace replay and split causes, and minimality through an independent table-filling oracle on the exhaustive small cases. CLI tests reload exported machines, check every example, and exercise invalid input, search limits, and existing-output protection. CI runs on Linux and Windows.

Library modules separate `dfa`, `moore`, `hopcroft`, `partition`, `trace`, `verify`, and `render`. Load through `Dfa::from_json`, call `reachable`, refine, and pass the resulting blocks to `partition::quotient`. Direct construction of `Dfa` requires maintaining its documented representation invariants; arbitrary partitions are not valid quotient inputs.
