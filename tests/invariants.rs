use dfa_minimization_partition_trace::{
    dfa::Dfa,
    hopcroft, moore,
    partition::{Refinement, quotient},
    trace::Cause,
    verify,
};
use std::collections::{BTreeMap, BTreeSet};

fn check_trace(dfa: &Dfa, refinement: &Refinement) {
    let trace = refinement.trace.as_ref().unwrap();
    let mut live: BTreeMap<usize, BTreeSet<String>> = trace
        .initial_partition
        .iter()
        .map(|b| (b.id, b.states.iter().cloned().collect()))
        .collect();
    let mut frozen = live.clone();
    let mut step = 0;
    let mut splits = 0;
    for event in &trace.events {
        assert!(event.step >= step);
        if event.step != step {
            frozen = live.clone();
            step = event.step;
        }
        let source: BTreeSet<_> = event.source.states.iter().cloned().collect();
        assert_eq!(live.remove(&event.source.id).unwrap(), source);
        let a = dfa
            .alphabet
            .iter()
            .position(|a| a == &event.symbol)
            .unwrap();
        let mut union = BTreeSet::new();
        for (index, child) in event.children.iter().enumerate() {
            assert!(!child.states.is_empty());
            for name in &child.states {
                assert!(union.insert(name.clone()), "children must be disjoint");
                let q = dfa.names.iter().position(|n| n == name).unwrap();
                let target = &dfa.names[dfa.transitions[q][a]];
                match &event.cause {
                    Cause::DestinationBlocks { destination_blocks } => {
                        assert!(frozen[&destination_blocks[index]].contains(target))
                    }
                    Cause::Splitter { splitter } => {
                        assert_eq!(
                            frozen[&splitter.id],
                            splitter.states.iter().cloned().collect()
                        );
                        assert_eq!(splitter.states.contains(target), index == 0);
                    }
                }
            }
            assert!(
                live.insert(child.id, child.states.iter().cloned().collect())
                    .is_none()
            );
        }
        assert_eq!(source, union);
        splits += event.children.len() - 1;
    }
    assert_eq!(splits, refinement.metrics.partition_splits);
    assert_eq!(live.len(), refinement.blocks.len());
    for (id, block) in refinement.blocks.iter().enumerate() {
        assert_eq!(
            live[&id],
            block.iter().map(|&q| dfa.names[q].clone()).collect()
        );
    }
}

// Independent table-filling oracle: no partition-refinement code is used here.
fn assert_minimal(dfa: &Dfa) {
    let n = dfa.names.len();
    let mut distinct = vec![vec![false; n]; n];
    for (p, row) in distinct.iter_mut().enumerate() {
        for (q, value) in row.iter_mut().enumerate() {
            *value = dfa.accepting[p] != dfa.accepting[q];
        }
    }
    loop {
        let mut changed = false;
        for p in 0..n {
            for q in p + 1..n {
                if !distinct[p][q]
                    && (0..dfa.alphabet.len())
                        .any(|a| distinct[dfa.transitions[p][a]][dfa.transitions[q][a]])
                {
                    distinct[p][q] = true;
                    distinct[q][p] = true;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    for (p, row) in distinct.iter().enumerate() {
        assert!(
            row.iter().skip(p + 1).all(|&marked| marked),
            "equivalent states remain"
        );
    }
}

#[test]
fn every_binary_dfa_with_up_to_three_states_is_minimized_and_traces_replay() {
    for n in 1usize..=3 {
        for encoded in 0..n.pow((2 * n) as u32) {
            let mut code = encoded;
            let transitions: Vec<Vec<usize>> = (0..n)
                .map(|_| {
                    (0..2)
                        .map(|_| {
                            let target = code % n;
                            code /= n;
                            target
                        })
                        .collect()
                })
                .collect();
            for mask in 0..(1 << n) {
                let dfa = Dfa {
                    alphabet: vec!["a".into(), "b".into()],
                    names: (0..n).map(|q| format!("q{q}")).collect(),
                    start: 0,
                    accepting: (0..n).map(|q| mask & (1 << q) != 0).collect(),
                    transitions: transitions.clone(),
                }
                .reachable()
                .0;
                let moore = moore::refine(&dfa, true);
                let hopcroft = hopcroft::refine(&dfa, true);
                check_trace(&dfa, &moore);
                check_trace(&dfa, &hopcroft);
                let left = quotient(&dfa, &moore.blocks).dfa;
                let right = quotient(&dfa, &hopcroft.blocks).dfa;
                assert!(verify::structurally_equal(&left, &right));
                assert!(verify::equivalent(&dfa, &left).unwrap().equivalent);
                assert_minimal(&left);
            }
        }
    }
}

#[test]
fn unicode_symbols_and_reordered_states_keep_the_same_canonical_machine() {
    let left = Dfa::from_json(r#"{"alphabet":["β","α"],"states":["終","始"],"start":"始","accepting":["終"],"transitions":{"始":{"α":"終","β":"始"},"終":{"α":"始","β":"終"}}}"#).unwrap();
    let right = Dfa::from_json(r#"{"alphabet":["α","β"],"states":["始","終"],"start":"始","accepting":["終"],"transitions":{"始":{"α":"終","β":"始"},"終":{"α":"始","β":"終"}}}"#).unwrap();
    assert!(left.accepts("ββα").unwrap());
    let left = left.reachable().0;
    let right = right.reachable().0;
    assert!(verify::structurally_equal(
        &quotient(&left, &moore::refine(&left, false).blocks).dfa,
        &quotient(&right, &hopcroft::refine(&right, false).blocks).dfa
    ));
}
