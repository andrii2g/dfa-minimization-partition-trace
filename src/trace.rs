use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Block {
    pub id: usize,
    pub states: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Cause {
    /// One destination block per child, all from the partition at the start of this round.
    DestinationBlocks { destination_blocks: Vec<usize> },
    /// The child at index 0 enters this splitter; index 1 does not.
    Splitter { splitter: Block },
}

#[derive(Debug, Clone, Serialize)]
pub struct SplitEvent {
    pub step: usize,
    pub symbol: String,
    pub source: Block,
    pub children: Vec<Block>,
    pub cause: Cause,
}

#[derive(Debug, Clone, Serialize)]
pub struct Trace {
    pub algorithm: String,
    pub initial_partition: Vec<Block>,
    pub events: Vec<SplitEvent>,
}
