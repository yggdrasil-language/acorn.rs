//! Execution limits for codec transforms.

use crate::error::Ratio;

/// Limits applied during codec execution.
///
/// Different transforms read only the fields relevant to their family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodecLimits {
    /// Maximum input bytes that may be consumed.
    pub max_input_bytes: Option<u64>,
    /// Maximum output bytes that may be produced.
    pub max_output_bytes: Option<u64>,
    /// Maximum output-to-input expansion ratio.
    pub max_expansion_ratio: Option<Ratio>,
    /// Maximum decode steps (symbols, iterations).
    pub max_steps: Option<u64>,
    /// Maximum Huffman symbols in a dynamic table.
    pub max_huffman_symbols: Option<u64>,
    /// Maximum back-reference distance.
    pub max_distance: Option<u32>,
    /// Maximum number of deflate blocks.
    pub max_block_count: Option<u64>,
}

impl Default for CodecLimits {
    fn default() -> Self {
        Self::new()
    }
}

impl CodecLimits {
    /// Creates an empty limit set (no restrictions).
    #[inline]
    pub const fn new() -> Self {
        Self {
            max_input_bytes: None,
            max_output_bytes: None,
            max_expansion_ratio: None,
            max_steps: None,
            max_huffman_symbols: None,
            max_distance: None,
            max_block_count: None,
        }
    }

    /// Sets the maximum input byte count.
    #[inline]
    pub const fn with_max_input_bytes(mut self, limit: u64) -> Self {
        self.max_input_bytes = Some(limit);
        self
    }

    /// Sets the maximum output byte count.
    #[inline]
    pub const fn with_max_output_bytes(mut self, limit: u64) -> Self {
        self.max_output_bytes = Some(limit);
        self
    }

    /// Sets the maximum expansion ratio.
    #[inline]
    pub const fn with_max_expansion_ratio(mut self, ratio: Ratio) -> Self {
        self.max_expansion_ratio = Some(ratio);
        self
    }

    /// Sets the maximum decode step count.
    #[inline]
    pub const fn with_max_steps(mut self, limit: u64) -> Self {
        self.max_steps = Some(limit);
        self
    }

    /// Sets the maximum Huffman symbol count.
    #[inline]
    pub const fn with_max_huffman_symbols(mut self, limit: u64) -> Self {
        self.max_huffman_symbols = Some(limit);
        self
    }

    /// Sets the maximum back-reference distance.
    #[inline]
    pub const fn with_max_distance(mut self, limit: u32) -> Self {
        self.max_distance = Some(limit);
        self
    }

    /// Sets the maximum deflate block count.
    #[inline]
    pub const fn with_max_block_count(mut self, limit: u64) -> Self {
        self.max_block_count = Some(limit);
        self
    }

    /// Returns the output byte limit, defaulting to unlimited.
    #[inline]
    pub fn output_limit(&self) -> u64 {
        self.max_output_bytes.unwrap_or(u64::MAX)
    }

    /// Checks whether producing `output` bytes from `input` bytes violates the expansion ratio.
    pub fn check_expansion(&self, input: u64, output: u64) -> bool {
        match self.max_expansion_ratio {
            None => true,
            Some(ratio) if ratio.denominator == 0 => false,
            Some(ratio) => output * ratio.denominator <= input * ratio.numerator,
        }
    }
}
