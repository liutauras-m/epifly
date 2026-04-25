pub const MAX_PROMPT_TOKENS: usize = 128_000;
pub const MAX_RESPONSE_TOKENS: usize = 16_384;
pub const MAX_CAPABILITY_SIZE_BYTES: usize = 50 * 1024 * 1024; // 50 MB
pub const MAX_WASM_SIZE_BYTES: usize = 10 * 1024 * 1024;       // 10 MB
pub const REQUEST_TIMEOUT_SECS: u64 = 120;
pub const MAX_CONCURRENT_AGENTS: usize = 64;
