use crate::{
    dfa::Dfa,
    partition::{Metrics, Refinement, initial_partition, membership, named_block, new_trace},
    trace::{Cause, SplitEvent},
};
use std::collections::BTreeMap;

/// Synchronous Moore refinement. Every symbol in a round uses the frozen old partition.
pub fn refine(dfa: &Dfa, tracing: bool) -> Refinement {
    let mut blocks = initial_partition(dfa);
    let mut trace = tracing.then(|| new_trace(dfa, "moore", &blocks));
    let mut metrics = Metrics::default();
    let mut round = 0;
    loop {
        round += 1;
        let old_membership = membership(&blocks, dfa.names.len());
        let old_count = blocks.len();
        for a in 0..dfa.alphabet.len() {
            let count_before_symbol = blocks.len();
            for id in 0..count_before_symbol {
                let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
                for &q in &blocks[id] {
                    metrics.transition_examinations += 1;
                    groups
                        .entry(old_membership[dfa.transitions[q][a]])
                        .or_default()
                        .push(q);
                }
                if groups.len() <= 1 {
                    continue;
                }
                let source = tracing.then(|| named_block(dfa, id, &blocks[id]));
                let mut children = Vec::new();
                let mut destinations = Vec::new();
                metrics.partition_splits += groups.len() - 1;
                for (index, (destination, states)) in groups.into_iter().enumerate() {
                    let child_id = if index == 0 { id } else { blocks.len() };
                    if tracing {
                        children.push(named_block(dfa, child_id, &states));
                        destinations.push(destination);
                    }
                    if index == 0 {
                        blocks[id] = states;
                    } else {
                        blocks.push(states);
                    }
                }
                if let Some(trace) = &mut trace {
                    trace.events.push(SplitEvent {
                        step: round,
                        symbol: dfa.alphabet[a].clone(),
                        source: source.unwrap(),
                        children,
                        cause: Cause::DestinationBlocks {
                            destination_blocks: destinations,
                        },
                    });
                }
            }
        }
        if blocks.len() == old_count {
            break;
        }
    }
    metrics.moore_rounds = Some(round);
    Refinement {
        blocks,
        metrics,
        trace,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::partition::quotient;

    #[test]
    fn propagates_distinctions_one_round_at_a_time() {
        let dfa = Dfa {
            alphabet: vec!["a".into()],
            names: (0..5).map(|i| i.to_string()).collect(),
            start: 0,
            accepting: vec![false, false, false, false, true],
            transitions: vec![vec![1], vec![2], vec![3], vec![4], vec![4]],
        };
        let result = refine(&dfa, true);
        assert_eq!(result.blocks.len(), 5);
        assert_eq!(result.metrics.moore_rounds, Some(4));
        assert_eq!(result.metrics.partition_splits, 3);
        assert_eq!(result.metrics.transition_examinations, 20);
        assert_eq!(result.trace.unwrap().events.len(), 3);
        assert_eq!(
            quotient(&dfa, &result.blocks).dfa.transitions,
            dfa.transitions
        );
    }

    #[test]
    fn merges_equivalent_states() {
        let dfa = Dfa {
            alphabet: vec!["a".into()],
            names: vec!["s".into(), "f1".into(), "f2".into()],
            start: 0,
            accepting: vec![false, true, true],
            transitions: vec![vec![1], vec![2], vec![1]],
        };
        let result = refine(&dfa, false);
        let minimized = quotient(&dfa, &result.blocks);
        assert_eq!(minimized.dfa.names.len(), 2);
        assert_eq!(minimized.members[1], ["f1", "f2"]);
    }
}
