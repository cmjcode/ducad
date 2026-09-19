//! Kontrak backend model dan backend tiruan untuk tes.

use std::collections::VecDeque;

/// Backend model bahasa. `complete` boleh lambat (detik): panggil dari
/// thread latar, tidak pernah dari UI thread.
pub trait AssistBackend: Send {
    fn name(&self) -> &str;
    /// `true` bila inferensi berjalan di perangkat (tanpa jaringan).
    fn is_on_device(&self) -> bool;
    fn complete(&mut self, system: &str, user: &str, max_tokens: usize) -> anyhow::Result<String>;
}

/// Backend berbalasan terskrip; mencatat setiap pesan user yang diterima.
#[derive(Debug, Default)]
pub struct MockBackend {
    replies: VecDeque<String>,
    pub calls: Vec<String>,
}

impl MockBackend {
    pub fn new<I, S>(replies: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            replies: replies.into_iter().map(Into::into).collect(),
            calls: Vec::new(),
        }
    }
}

impl AssistBackend for MockBackend {
    fn name(&self) -> &str {
        "mock"
    }

    fn is_on_device(&self) -> bool {
        true
    }

    fn complete(&mut self, _system: &str, user: &str, _max_tokens: usize) -> anyhow::Result<String> {
        self.calls.push(user.to_string());
        self.replies
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("balasan terskrip habis"))
    }
}
