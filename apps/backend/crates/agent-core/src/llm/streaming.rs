use crate::llm::error::LlmError;
use crate::llm::types::{LlmChunk, LlmStream};
use futures::StreamExt;

/// Convert a raw byte stream (e.g. from `reqwest`) that emits OpenAI-compatible
/// SSE lines into a typed `LlmStream`.
///
/// Format expected per chunk:
/// ```text
/// data: {"choices":[{"delta":{"content":"hello"},"finish_reason":null}]}
/// data: [DONE]
/// ```
pub fn openai_sse_to_stream(
    byte_stream: impl futures::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
) -> LlmStream {
    let stream = byte_stream.flat_map(|result| {
        let items: Vec<Result<LlmChunk, LlmError>> = match result {
            Err(e) => vec![Err(LlmError::Provider {
                provider: "openai_sse",
                message: e.to_string(),
            })],
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes);
                text.lines()
                    .filter(|line| line.starts_with("data: ") && *line != "data: [DONE]")
                    .filter_map(|line| {
                        let json_str = &line["data: ".len()..];
                        let v: serde_json::Value = serde_json::from_str(json_str).ok()?;
                        let delta = v["choices"][0]["delta"]["content"]
                            .as_str()
                            .unwrap_or("")
                            .to_string();
                        let finish_reason = v["choices"][0]["finish_reason"]
                            .as_str()
                            .map(str::to_string);
                        Some(Ok(LlmChunk { delta, finish_reason }))
                    })
                    .collect()
            }
        };
        futures::stream::iter(items)
    });

    Box::pin(stream)
}
