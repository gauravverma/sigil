//! Token counters used by `sigil benchmark`.
//!
//! Default is the `bytes / 4` proxy — a stable approximation of modern
//! tokenizers at ±20%, accurate enough for relative comparisons and with
//! no runtime dependency. The `tokenizer` cargo feature pulls in
//! `tiktoken-rs` and unlocks BPE-accurate counts for
//! `cl100k_base` (GPT-3.5/4), `o200k_base` (GPT-4o/o3), and
//! `p50k_base` (legacy GPT-3.5).
//!
//! Anthropic's tokenizer isn't bundled because there's no pure-Rust
//! implementation yet — the closest thing is `tiktoken-rs::o200k_base`,
//! which tokenizes comparably on code corpora (difference < 5% in our
//! spot checks). Real Claude-accurate counting needs the Anthropic API.

use anyhow::Result;
#[cfg(not(feature = "tokenizer"))]
use anyhow::bail;

/// Select a token counter. Keep this enum closed so `--tokenizer` validation
/// is centralized; add variants when new corpora need them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tokenizer {
    /// `bytes / 4` heuristic. Always available.
    Proxy,
    /// OpenAI cl100k_base — GPT-3.5, GPT-4, GPT-4-turbo.
    Cl100k,
    /// OpenAI o200k_base — GPT-4o, o3. Closest match to Claude on code.
    O200k,
    /// OpenAI p50k_base — legacy GPT-3.5 / codex.
    P50k,
}

impl Default for Tokenizer {
    fn default() -> Self {
        Self::Proxy
    }
}

impl Tokenizer {
    /// Parse a CLI flag value. Returns `None` on unknown values so the
    /// caller can surface a helpful error listing the valid options.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "" | "proxy" | "bytes" => Some(Self::Proxy),
            "cl100k_base" | "cl100k" => Some(Self::Cl100k),
            "o200k_base" | "o200k" => Some(Self::O200k),
            "p50k_base" | "p50k" => Some(Self::P50k),
            _ => None,
        }
    }

    /// Human-readable label for inclusion in benchmark output.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Proxy => "bytes/4 proxy",
            Self::Cl100k => "cl100k_base (GPT-3.5/4)",
            Self::O200k => "o200k_base (GPT-4o/o3)",
            Self::P50k => "p50k_base (legacy)",
        }
    }

    /// True when `count` will call into an accurate BPE tokenizer rather
    /// than the proxy. Lets callers annotate output truthfully.
    pub fn is_accurate(&self) -> bool {
        !matches!(self, Self::Proxy)
    }

    /// Count tokens in `text` using the selected tokenizer. Errors only
    /// when the `tokenizer` feature isn't compiled in yet the caller
    /// asked for a BPE variant.
    pub fn count(&self, text: &str) -> Result<usize> {
        match self {
            Self::Proxy => Ok(proxy_count(text)),
            #[cfg(feature = "tokenizer")]
            Self::Cl100k => Ok(tiktoken_rs::cl100k_base()?.encode_with_special_tokens(text).len()),
            #[cfg(feature = "tokenizer")]
            Self::O200k => Ok(tiktoken_rs::o200k_base()?.encode_with_special_tokens(text).len()),
            #[cfg(feature = "tokenizer")]
            Self::P50k => Ok(tiktoken_rs::p50k_base()?.encode_with_special_tokens(text).len()),
            #[cfg(not(feature = "tokenizer"))]
            _ => bail!(
                "sigil was built without the `tokenizer` feature — rebuild with \
                 `cargo install sigil --features tokenizer` to use `{}`.",
                self.label()
            ),
        }
    }
}

/// The proxy tokenizer — 1 token ≈ 4 bytes. Infallible, always-on.
/// Shared with map.rs and context.rs for budget gating.
pub fn proxy_count(s: &str) -> usize {
    (s.len() + 3) / 4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_accepts_documented_aliases() {
        assert_eq!(Tokenizer::parse(""), Some(Tokenizer::Proxy));
        assert_eq!(Tokenizer::parse("proxy"), Some(Tokenizer::Proxy));
        assert_eq!(Tokenizer::parse("bytes"), Some(Tokenizer::Proxy));
        assert_eq!(Tokenizer::parse("cl100k"), Some(Tokenizer::Cl100k));
        assert_eq!(Tokenizer::parse("cl100k_base"), Some(Tokenizer::Cl100k));
        assert_eq!(Tokenizer::parse("O200K_BASE"), Some(Tokenizer::O200k));
        assert_eq!(Tokenizer::parse("gibberish"), None);
    }

    #[test]
    fn proxy_count_matches_prior_benchmark_heuristic() {
        assert_eq!(proxy_count(""), 0);
        assert_eq!(proxy_count("a"), 1);
        assert_eq!(proxy_count("abcd"), 1);
        assert_eq!(proxy_count("abcde"), 2);
    }

    #[test]
    fn proxy_always_works_without_feature() {
        // Regression guard — the default path must never error, since most
        // consumers run without the `tokenizer` feature compiled in.
        assert!(Tokenizer::Proxy.count("hello world").is_ok());
        assert!(!Tokenizer::Proxy.is_accurate());
    }

    #[cfg(feature = "tokenizer")]
    #[test]
    fn bpe_counts_are_finite_and_non_zero() {
        // "Hello, world!" hits multiple BPE splits in every encoding. We
        // don't assert exact token counts (those depend on the vocabulary
        // version), just that the encoder returns plausible numbers and
        // marks itself as accurate.
        for enc in [Tokenizer::Cl100k, Tokenizer::O200k, Tokenizer::P50k] {
            let n = enc.count("Hello, world!").unwrap();
            assert!(n > 0 && n < 20, "{enc:?} produced implausible count {n}");
            assert!(enc.is_accurate());
        }
    }
}
