use crate::{
    dfa::Dfa,
    trace::{Block, Trace},
};
use serde::Serialize;
use std::collections::VecDeque;

#[derive(Debug, Clone, Default, Serialize)]
pub struct Metrics {
    pub moore_rounds: Option<usize>,
    pub hopcroft_steps: Option<usize>,
    pub partition_splits: usize,
    pub transition_examinations: u64,
}

#[derive(Debug)]
pub struct Refinement {
    pub blocks: Vec<Vec<usize>>,
    pub metrics: Metrics,
    pub trace: Option<Trace>,
}

pub fn initial_partition(dfa: &Dfa) -> Vec<Vec<usize>> {
    [false, true]
        .into_iter()
        .map(|accepting| {
            (0..dfa.names.len())
                .filter(|&q| dfa.accepting[q] == accepting)
                .collect::<Vec<_>>()
        })
        .filter(|block| !block.is_empty())
        .collect()
}

pub fn membership(blocks: &[Vec<usize>], states: usize) -> Vec<usize> {
    let mut map = vec![0; states];
    for (id, block) in blocks.iter().enumerate() {
        for &q in block {
            map[q] = id;
        }
    }
    map
}

pub fn named_block(dfa: &Dfa, id: usize, states: &[usize]) -> Block {
    let mut names: Vec<_> = states.iter().map(|&q| dfa.names[q].clone()).collect();
    names.sort();
    Block { id, states: names }
}

pub fn new_trace(dfa: &Dfa, algorithm: &str, blocks: &[Vec<usize>]) -> Trace {
    Trace {
        algorithm: algorithm.to_owned(),
        initial_partition: blocks
            .iter()
            .enumerate()
            .map(|(id, states)| named_block(dfa, id, states))
            .collect(),
        events: Vec::new(),
    }
}

#[derive(Debug)]
pub struct Minimized {
    pub dfa: Dfa,
    /// Members indexed by canonical minimized-state ID.
    pub members: Vec<Vec<String>>,
}

/// Construct the quotient in canonical BFS order, using the sorted alphabet.
pub fn quotient(dfa: &Dfa, blocks: &[Vec<usize>]) -> Minimized {
    let membership = membership(blocks, dfa.names.len());
    let start = membership[dfa.start];
    let mut canonical = vec![usize::MAX; blocks.len()];
    canonical[start] = 0;
    let mut count = 1;
    let mut queue = VecDeque::from([start]);
    let mut accepting = Vec::new();
    let mut transitions = Vec::new();
    let mut members = Vec::new();
    while let Some(block) = queue.pop_front() {
        let representative = blocks[block][0];
        accepting.push(dfa.accepting[representative]);
        let mut names: Vec<_> = blocks[block]
            .iter()
            .map(|&q| dfa.names[q].clone())
            .collect();
        names.sort();
        members.push(names);
        let mut row = Vec::new();
        for &target in &dfa.transitions[representative] {
            let target_block = membership[target];
            if canonical[target_block] == usize::MAX {
                canonical[target_block] = count;
                count += 1;
                queue.push_back(target_block);
            }
            row.push(canonical[target_block]);
        }
        transitions.push(row);
    }
    Minimized {
        dfa: Dfa {
            alphabet: dfa.alphabet.clone(),
            names: (0..count).map(|q| format!("M{q}")).collect(),
            start: 0,
            accepting,
            transitions,
        },
        members,
    }
}
