// copyright (C) 2022-2023 Ula Shipman <ula.hello@mailbox.org>
// licensed under GPL-3.0-or-later

use tokio::sync::{mpsc, RwLock};
use tokio::task::{self, JoinHandle};
use tokio_util::sync::CancellationToken;

use alloc::sync::Arc;
use std::path::PathBuf;

use crate::bar::BarMsg;
use crate::block::Block;
use crate::config::TomlBlock;
use crate::protocol::Body;

#[derive(Debug)]
pub struct Blocks {
    inner: Vec<(JoinHandle<()>, CancellationToken, Arc<RwLock<Body>>)>,
    bar_tx: mpsc::Sender<BarMsg>,
}

impl Blocks {
    pub const fn new(bar_tx: mpsc::Sender<BarMsg>) -> Self {
        Self {
            inner: Vec::new(),
            bar_tx,
        }
    }

    pub async fn remove_all(&mut self) {
        for (handle, token, _body) in core::mem::take(&mut self.inner) {
            token.cancel();
            handle.await.unwrap();
        }
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn add_all<B: Iterator<Item = TomlBlock> + ExactSizeIterator>(
        &mut self,
        blocks: B,
        global_body: Arc<Body>,
        command_dir: Arc<PathBuf>,
    ) {
        assert!(self.inner.is_empty());
        let num_blocks = blocks.len();
        for (id, toml) in blocks.enumerate() {
            let (block, token) = Block::new(
                toml,
                Arc::clone(&global_body),
                Arc::clone(&command_dir),
                self.bar_tx.clone(),
                id,
                num_blocks,
            );
            let body = block.body();
            let handle = task::spawn(async move { block.listen().await });
            self.inner.push((handle, token, body));
        }
    }

    pub fn iter(
        &self,
    ) -> core::slice::Iter<(JoinHandle<()>, CancellationToken, Arc<RwLock<Body>>)> {
        self.inner.iter()
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }
}
