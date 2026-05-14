use std::sync::{Arc, Mutex};

use futures::stream::{self, BoxStream, StreamExt};

use crate::client::{AiAttachment, AiChunk, AiClient, AiError};

#[derive(Clone)]
pub struct FakeAiClient {
    program: Program,
    prompts: Arc<Mutex<Vec<String>>>,
    attachments: Arc<Mutex<Vec<Vec<AiAttachment>>>>,
}

#[derive(Clone)]
enum Program {
    Chunks(Vec<String>),
    Failure(String),
}

impl FakeAiClient {
    pub fn new(chunks: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            program: Program::Chunks(chunks.into_iter().map(Into::into).collect()),
            prompts: Arc::default(),
            attachments: Arc::default(),
        }
    }

    pub fn failing(message: impl Into<String>) -> Self {
        Self {
            program: Program::Failure(message.into()),
            prompts: Arc::default(),
            attachments: Arc::default(),
        }
    }

    pub fn prompts(&self) -> Vec<String> {
        self.prompts.lock().expect("prompts mutex").clone()
    }

    pub fn attachments(&self) -> Vec<Vec<AiAttachment>> {
        self.attachments.lock().expect("attachments mutex").clone()
    }
}

impl AiClient for FakeAiClient {
    fn complete(
        &self,
        prompt: String,
        attachments: Vec<AiAttachment>,
    ) -> BoxStream<'static, Result<AiChunk, AiError>> {
        self.prompts.lock().expect("prompts mutex").push(prompt);
        self.attachments
            .lock()
            .expect("attachments mutex")
            .push(attachments);

        match self.program.clone() {
            Program::Chunks(chunks) => {
                let full_text: String = chunks.concat();
                let mut items: Vec<Result<AiChunk, AiError>> =
                    chunks.into_iter().map(|c| Ok(AiChunk::Delta(c))).collect();
                items.push(Ok(AiChunk::Done { full_text }));
                stream::iter(items).boxed()
            }
            Program::Failure(msg) => stream::iter(vec![Err(AiError::Protocol(msg))]).boxed(),
        }
    }
}
