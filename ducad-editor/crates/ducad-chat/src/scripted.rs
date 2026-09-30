//! Model berbalasan terskrip untuk tes (pola `MockBackend` ducad-assist).

use std::collections::VecDeque;
use std::sync::atomic::AtomicBool;

use crate::{ChatModel, ConvMessage, ModelTurn, TurnRequest};

#[derive(Default)]
pub struct ScriptedModel {
    turns: VecDeque<ModelTurn>,
    /// Salinan percakapan yang diterima setiap giliran.
    pub seen: Vec<Vec<ConvMessage>>,
}

impl ScriptedModel {
    pub fn new(turns: impl IntoIterator<Item = ModelTurn>) -> Self {
        Self {
            turns: turns.into_iter().collect(),
            seen: Vec::new(),
        }
    }
}

impl ChatModel for ScriptedModel {
    fn name(&self) -> String {
        "scripted".into()
    }

    fn turn(
        &mut self,
        req: &TurnRequest,
        on_text: &mut dyn FnMut(&str),
        _cancel: &AtomicBool,
    ) -> anyhow::Result<ModelTurn> {
        self.seen.push(req.conv.to_vec());
        let t = self
            .turns
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("balasan terskrip habis"))?;
        if !t.text.is_empty() {
            on_text(&t.text);
        }
        Ok(t)
    }
}
