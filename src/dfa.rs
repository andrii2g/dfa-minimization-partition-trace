use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Human-readable, complete deterministic automaton. Symbols are single Unicode characters.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DfaJson {
    pub alphabet: Vec<String>,
    pub states: Vec<String>,
    pub start: String,
    pub accepting: Vec<String>,
    pub transitions: BTreeMap<String, BTreeMap<String, String>>,
}

#[derive(Debug, Clone)]
pub struct Dfa {
    pub alphabet: Vec<String>,
    pub names: Vec<String>,
    pub start: usize,
    pub accepting: Vec<bool>,
    pub transitions: Vec<Vec<usize>>,
}

impl TryFrom<DfaJson> for Dfa {
    type Error = anyhow::Error;

    fn try_from(input: DfaJson) -> Result<Self> {
        ensure!(!input.states.is_empty(), "states must not be empty");
        let indices: BTreeMap<_, _> = input
            .states
            .iter()
            .enumerate()
            .map(|(i, s)| (s.clone(), i))
            .collect();
        ensure!(
            indices.len() == input.states.len(),
            "state names must be unique"
        );
        ensure!(
            input.states.iter().all(|s| !s.is_empty()),
            "state names must not be empty"
        );
        let mut alphabet = input.alphabet;
        alphabet.sort();
        ensure!(
            alphabet.len() <= 16,
            "alphabet must contain at most 16 symbols"
        );
        ensure!(
            alphabet.iter().all(|s| s.chars().count() == 1),
            "each symbol must be one Unicode character"
        );
        ensure!(
            alphabet.windows(2).all(|w| w[0] != w[1]),
            "alphabet symbols must be unique"
        );
        let Some(&start) = indices.get(&input.start) else {
            bail!("unknown start state {:?}", input.start)
        };
        let mut accepting = vec![false; indices.len()];
        for name in &input.accepting {
            let Some(&index) = indices.get(name) else {
                bail!("unknown accepting state {name:?}")
            };
            ensure!(!accepting[index], "duplicate accepting state {name:?}");
            accepting[index] = true;
        }
        for name in input.transitions.keys() {
            ensure!(
                indices.contains_key(name),
                "transition row for unknown state {name:?}"
            );
        }
        let symbols: BTreeSet<_> = alphabet.iter().collect();
        let mut transitions = Vec::with_capacity(indices.len());
        for name in &input.states {
            let Some(row) = input.transitions.get(name) else {
                bail!("missing transition row for {name:?}")
            };
            for symbol in row.keys() {
                ensure!(
                    symbols.contains(symbol),
                    "unknown symbol {symbol:?} in transition row {name:?}"
                );
            }
            let mut targets = Vec::with_capacity(alphabet.len());
            for symbol in &alphabet {
                let Some(target) = row.get(symbol) else {
                    bail!("missing transition from {name:?} on {symbol:?}")
                };
                let Some(&index) = indices.get(target) else {
                    bail!("unknown target {target:?} from {name:?} on {symbol:?}")
                };
                targets.push(index);
            }
            transitions.push(targets);
        }
        Ok(Self {
            alphabet,
            names: input.states,
            start,
            accepting,
            transitions,
        })
    }
}

impl Dfa {
    pub fn from_json(text: &str) -> Result<Self> {
        serde_json::from_str::<DfaJson>(text)?.try_into()
    }

    pub fn to_json(&self) -> DfaJson {
        DfaJson {
            alphabet: self.alphabet.clone(),
            states: self.names.clone(),
            start: self.names[self.start].clone(),
            accepting: self
                .names
                .iter()
                .zip(&self.accepting)
                .filter(|(_, yes)| **yes)
                .map(|(name, _)| name.clone())
                .collect(),
            transitions: self
                .names
                .iter()
                .enumerate()
                .map(|(q, name)| {
                    (
                        name.clone(),
                        self.alphabet
                            .iter()
                            .enumerate()
                            .map(|(a, symbol)| {
                                (symbol.clone(), self.names[self.transitions[q][a]].clone())
                            })
                            .collect(),
                    )
                })
                .collect(),
        }
    }

    pub fn accepts(&self, word: &str) -> Result<bool> {
        let mut state = self.start;
        for symbol in word.chars() {
            let Ok(a) = self.alphabet.binary_search(&symbol.to_string()) else {
                bail!("word contains unknown symbol {symbol:?}")
            };
            state = self.transitions[state][a];
        }
        Ok(self.accepting[state])
    }

    /// BFS ordering also gives deterministic IDs independent of the input state ordering.
    pub fn reachable(&self) -> (Self, Vec<String>) {
        let mut order = Vec::new();
        let mut map = vec![usize::MAX; self.names.len()];
        map[self.start] = 0;
        let mut queue = VecDeque::from([self.start]);
        let mut discovered = 1;
        while let Some(q) = queue.pop_front() {
            order.push(q);
            for &target in &self.transitions[q] {
                if map[target] == usize::MAX {
                    map[target] = discovered;
                    discovered += 1;
                    queue.push_back(target);
                }
            }
        }
        let removed = self
            .names
            .iter()
            .enumerate()
            .filter(|(q, _)| map[*q] == usize::MAX)
            .map(|(_, name)| name.clone())
            .collect();
        (
            Self {
                alphabet: self.alphabet.clone(),
                names: order.iter().map(|&q| self.names[q].clone()).collect(),
                start: 0,
                accepting: order.iter().map(|&q| self.accepting[q]).collect(),
                transitions: order
                    .iter()
                    .map(|&q| self.transitions[q].iter().map(|&t| map[t]).collect())
                    .collect(),
            },
            removed,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{"alphabet":["b","a"],"states":["s","f","dead"],"start":"s","accepting":["f"],"transitions":{"s":{"a":"f","b":"s"},"f":{"a":"f","b":"s"},"dead":{"a":"dead","b":"dead"}}}"#;

    #[test]
    fn simulation_reachability_and_roundtrip() {
        let dfa = Dfa::from_json(SAMPLE).unwrap();
        assert!(!dfa.accepts("").unwrap());
        assert!(dfa.accepts("bba").unwrap());
        assert!(!dfa.accepts("aab").unwrap());
        assert!(dfa.accepts("c").is_err());
        let (reachable, removed) = dfa.reachable();
        assert_eq!(removed, ["dead"]);
        assert_eq!(reachable.names, ["s", "f"]);
        let copy = Dfa::from_json(&serde_json::to_string(&reachable.to_json()).unwrap()).unwrap();
        assert_eq!(copy.transitions, reachable.transitions);
    }

    #[test]
    fn rejects_invalid_input() {
        for invalid in [
            SAMPLE.replace("\"b\":\"s\"", "\"c\":\"s\""),
            SAMPLE.replace("\"start\":\"s\"", "\"start\":\"missing\""),
            SAMPLE.replace("[\"b\",\"a\"]", "[\"a\",\"a\"]"),
            SAMPLE.replace("\"a\":\"f\"", "\"a\":\"missing\""),
        ] {
            assert!(Dfa::from_json(&invalid).is_err());
        }
    }
}
