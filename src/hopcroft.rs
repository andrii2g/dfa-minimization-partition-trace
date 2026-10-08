use crate::{
    dfa::Dfa,
    partition::{Metrics, Refinement, initial_partition, membership, named_block, new_trace},
    trace::{Cause, SplitEvent},
};
use std::collections::VecDeque;

/// Hopcroft refinement with reverse edges and constant-time removal from a block.
/// With tracing disabled, splitting never copies or scans the untouched complement.
pub fn refine(dfa: &Dfa, tracing: bool) -> Refinement {
    let n = dfa.names.len();
    let k = dfa.alphabet.len();
    let mut blocks = initial_partition(dfa);
    let mut trace = tracing.then(|| new_trace(dfa, "hopcroft", &blocks));
    let mut metrics = Metrics::default();
    let mut state_block = membership(&blocks, n);
    let mut position = vec![0; n];
    for block in &blocks {
        for (index, &q) in block.iter().enumerate() {
            position[q] = index;
        }
    }
    let mut predecessors = vec![vec![Vec::new(); n]; k];
    for (q, row) in dfa.transitions.iter().enumerate() {
        for (a, &target) in row.iter().enumerate() {
            predecessors[a][target].push(q);
            metrics.transition_examinations += 1;
        }
    }
    let mut queue = VecDeque::new();
    let mut pending = vec![vec![false; k]; blocks.len()];
    if blocks.len() == 2 {
        let smaller = if blocks[0].len() <= blocks[1].len() {
            0
        } else {
            1
        };
        for (a, is_pending) in pending[smaller].iter_mut().enumerate() {
            *is_pending = true;
            queue.push_back((smaller, a));
        }
    }
    let mut buckets = vec![Vec::new(); blocks.len()];
    let mut steps = 0;
    while let Some((splitter_id, a)) = queue.pop_front() {
        steps += 1;
        pending[splitter_id][a] = false;
        let splitter = tracing.then(|| named_block(dfa, splitter_id, &blocks[splitter_id]));
        let mut touched = Vec::new();
        // Capture the entire preimage before mutating any blocks (including the splitter).
        for &target in &blocks[splitter_id] {
            for &q in &predecessors[a][target] {
                metrics.transition_examinations += 1;
                let id = state_block[q];
                if buckets[id].is_empty() {
                    touched.push(id);
                }
                buckets[id].push(q);
            }
        }
        for id in touched {
            let selected = std::mem::take(&mut buckets[id]);
            if selected.len() == blocks[id].len() {
                continue;
            }
            let source = tracing.then(|| named_block(dfa, id, &blocks[id]));
            let new_id = blocks.len();
            for (index, &q) in selected.iter().enumerate() {
                let slot = position[q];
                blocks[id].swap_remove(slot);
                if slot < blocks[id].len() {
                    position[blocks[id][slot]] = slot;
                }
                position[q] = index;
                state_block[q] = new_id;
            }
            blocks.push(selected);
            buckets.push(Vec::new());
            pending.push(vec![false; k]);
            metrics.partition_splits += 1;
            if let Some(trace) = &mut trace {
                trace.events.push(SplitEvent {
                    step: steps,
                    symbol: dfa.alphabet[a].clone(),
                    source: source.unwrap(),
                    children: vec![
                        named_block(dfa, new_id, &blocks[new_id]),
                        named_block(dfa, id, &blocks[id]),
                    ],
                    cause: Cause::Splitter {
                        splitter: splitter.clone().unwrap(),
                    },
                });
            }
            for (symbol, _) in dfa.alphabet.iter().enumerate() {
                // Existing entries for the old block now represent its retained complement.
                let add = if pending[id][symbol] || blocks[new_id].len() <= blocks[id].len() {
                    new_id
                } else {
                    id
                };
                if !pending[add][symbol] {
                    pending[add][symbol] = true;
                    queue.push_back((add, symbol));
                }
            }
        }
    }
    metrics.hopcroft_steps = Some(steps);
    Refinement {
        blocks,
        metrics,
        trace,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{moore, partition::quotient, verify};

    #[test]
    fn matches_moore_on_seeded_automata() {
        let mut seed = 0x1234_5678u64;
        let mut random = |bound: usize| {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            ((seed >> 32) as usize) % bound
        };
        for n in 1..=30 {
            for k in 0..=4 {
                for _ in 0..12 {
                    let dfa = Dfa {
                        alphabet: (0..k)
                            .map(|a| char::from(b'a' + a as u8).to_string())
                            .collect(),
                        names: (0..n).map(|q| format!("q{q}")).collect(),
                        start: random(n),
                        accepting: (0..n).map(|_| random(2) == 1).collect(),
                        transitions: (0..n)
                            .map(|_| (0..k).map(|_| random(n)).collect())
                            .collect(),
                    }
                    .reachable()
                    .0;
                    let moore = moore::refine(&dfa, false);
                    let hopcroft = refine(&dfa, false);
                    let left = quotient(&dfa, &moore.blocks).dfa;
                    let right = quotient(&dfa, &hopcroft.blocks).dfa;
                    assert!(verify::structurally_equal(&left, &right), "n={n}, k={k}");
                    assert!(verify::equivalent(&dfa, &right).unwrap().equivalent);
                }
            }
        }
    }

    #[test]
    fn trace_does_not_change_results_or_counters() {
        let dfa = Dfa {
            alphabet: vec!["a".into()],
            names: (0..8).map(|q| q.to_string()).collect(),
            start: 0,
            accepting: (0..8).map(|q| q == 7).collect(),
            transitions: (0..8).map(|q| vec![(q + 1).min(7)]).collect(),
        };
        let traced = refine(&dfa, true);
        let plain = refine(&dfa, false);
        assert_eq!(traced.blocks, plain.blocks);
        assert_eq!(
            traced.metrics.transition_examinations,
            plain.metrics.transition_examinations
        );
        assert_eq!(traced.trace.unwrap().events.len(), 6);
        assert_eq!(plain.metrics.partition_splits, 6);
    }
}
