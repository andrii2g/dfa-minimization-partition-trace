use crate::dfa::Dfa;
use anyhow::{Result, bail, ensure};
use serde::Serialize;
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Serialize)]
pub struct Equivalence {
    pub equivalent: bool,
    /// A sequence of symbols; an empty sequence is the empty word.
    pub counterexample: Option<Vec<String>>,
    pub pairs_examined: usize,
}

/// Exact equivalence via the reachable product automaton. BFS finds a shortest witness.
pub fn equivalent(left: &Dfa, right: &Dfa) -> Result<Equivalence> {
    ensure!(
        left.alphabet == right.alphabet,
        "equivalence requires identical alphabets"
    );
    type Pair = (usize, usize);
    let start = (left.start, right.start);
    let mut parents: HashMap<Pair, Option<(Pair, usize)>> = HashMap::from([(start, None)]);
    let mut queue = VecDeque::from([start]);
    let mut pairs_examined = 0;
    while let Some(pair) = queue.pop_front() {
        pairs_examined += 1;
        if left.accepting[pair.0] != right.accepting[pair.1] {
            let mut word = Vec::new();
            let mut cursor = pair;
            while let Some((parent, a)) = parents[&cursor] {
                word.push(left.alphabet[a].clone());
                cursor = parent;
            }
            word.reverse();
            return Ok(Equivalence {
                equivalent: false,
                counterexample: Some(word),
                pairs_examined,
            });
        }
        for a in 0..left.alphabet.len() {
            let target = (left.transitions[pair.0][a], right.transitions[pair.1][a]);
            if let std::collections::hash_map::Entry::Vacant(entry) = parents.entry(target) {
                entry.insert(Some((pair, a)));
                queue.push_back(target);
            }
        }
    }
    Ok(Equivalence {
        equivalent: true,
        counterexample: None,
        pairs_examined,
    })
}

pub fn structurally_equal(left: &Dfa, right: &Dfa) -> bool {
    left.alphabet == right.alphabet
        && left.start == right.start
        && left.accepting == right.accepting
        && left.transitions == right.transitions
}

#[derive(Debug, Serialize)]
pub struct ExhaustiveCheck {
    pub equivalent: bool,
    pub max_word_length: u32,
    pub words_checked: u64,
    pub counterexample: Option<Vec<String>>,
}

/// Count the requested words before enumerating; overflow and excessive requests are errors.
pub fn word_count(alphabet_size: usize, max_length: u32, limit: u64) -> Result<u64> {
    let total = match alphabet_size {
        0 => 1,
        1 => u64::from(max_length) + 1,
        k => {
            let mut power = 1u64;
            let mut total = 1u64;
            for _ in 0..max_length {
                power = power
                    .checked_mul(k as u64)
                    .ok_or_else(|| anyhow::anyhow!("word count overflow"))?;
                total = total
                    .checked_add(power)
                    .ok_or_else(|| anyhow::anyhow!("word count overflow"))?;
                ensure!(
                    total <= limit,
                    "exhaustive comparison exceeds the {limit} word limit; reduce --max-word-length or increase --max-words"
                );
            }
            total
        }
    };
    ensure!(
        total <= limit,
        "exhaustive comparison needs {total} words, above the {limit} word limit"
    );
    Ok(total)
}

/// Depth-first enumeration with incremental simulation: no repeated prefix evaluation.
pub fn exhaustive(machines: &[&Dfa], max_length: u32, limit: u64) -> Result<ExhaustiveCheck> {
    let Some(first) = machines.first() else {
        bail!("at least one machine is required")
    };
    ensure!(
        machines.iter().all(|dfa| dfa.alphabet == first.alphabet),
        "exhaustive comparison requires identical alphabets"
    );
    word_count(first.alphabet.len(), max_length, limit)?;
    let mut states = vec![machines.iter().map(|dfa| dfa.start).collect::<Vec<_>>()];
    let mut next_symbols = vec![0];
    let mut word = Vec::new();
    let mut words_checked = 0;
    let mut just_entered = true;
    loop {
        let current = states.last().unwrap();
        if just_entered {
            words_checked += 1;
            let acceptance = first.accepting[current[0]];
            if machines
                .iter()
                .zip(current)
                .any(|(dfa, &q)| dfa.accepting[q] != acceptance)
            {
                return Ok(ExhaustiveCheck {
                    equivalent: false,
                    max_word_length: max_length,
                    words_checked,
                    counterexample: Some(
                        word.iter()
                            .map(|&a: &usize| first.alphabet[a].clone())
                            .collect(),
                    ),
                });
            }
        }
        let next = next_symbols.last_mut().unwrap();
        if word.len() >= max_length as usize || *next == first.alphabet.len() {
            states.pop();
            next_symbols.pop();
            if word.pop().is_none() {
                break;
            }
            just_entered = false;
        } else {
            let a = *next;
            *next += 1;
            let targets = machines
                .iter()
                .zip(current)
                .map(|(dfa, &q)| dfa.transitions[q][a])
                .collect();
            states.push(targets);
            next_symbols.push(0);
            word.push(a);
            just_entered = true;
        }
    }
    Ok(ExhaustiveCheck {
        equivalent: true,
        max_word_length: max_length,
        words_checked,
        counterexample: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chain(accepting: bool) -> Dfa {
        Dfa {
            alphabet: vec!["a".into(), "b".into()],
            names: vec!["s".into(), "t".into(), "u".into()],
            start: 0,
            accepting: vec![false, false, accepting],
            transitions: vec![vec![1, 0], vec![2, 0], vec![2, 2]],
        }
    }

    #[test]
    fn exact_check_finds_a_difference_beyond_short_word_limit() {
        let left = chain(true);
        let right = chain(false);
        assert!(exhaustive(&[&left, &right], 1, 100).unwrap().equivalent);
        let exact = equivalent(&left, &right).unwrap();
        assert_eq!(exact.counterexample.unwrap(), ["a", "a"]);
        assert!(!exhaustive(&[&left, &right], 2, 100).unwrap().equivalent);
    }

    #[test]
    fn handles_empty_word_and_counts_every_word() {
        let left = chain(true);
        assert_eq!(
            exhaustive(&[&left, &left], 8, 1000).unwrap().words_checked,
            511
        );
        assert_eq!(exhaustive(&[&left], 0, 1).unwrap().words_checked, 1);
        let mut right = left.clone();
        right.accepting[0] = true;
        assert_eq!(
            equivalent(&left, &right).unwrap().counterexample,
            Some(vec![])
        );
        assert!(word_count(2, 64, u64::MAX).is_err());
        assert!(word_count(2, 20, 1000).is_err());
        assert_eq!(word_count(0, u32::MAX, 1).unwrap(), 1);
        assert_eq!(word_count(1, 100, 101).unwrap(), 101);
    }
}
