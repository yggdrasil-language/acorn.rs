//! 32 KiB sliding window for deflate back-references.

const WINDOW_SIZE: usize = 32 * 1024;

/// Circular sliding window for deflate output.
pub struct SlidingWindow {
    buffer: Vec<u8>,
    pos: usize,
    filled: usize,
}

impl SlidingWindow {
    /// Creates an empty sliding window.
    pub fn new() -> Self {
        Self { buffer: vec![0u8; WINDOW_SIZE], pos: 0, filled: 0 }
    }

    /// Number of bytes currently available for back-references.
    pub fn len(&self) -> usize {
        self.filled
    }

    /// Whether the window is empty.
    pub fn is_empty(&self) -> bool {
        self.filled == 0
    }

    /// Whether `distance` resolves inside the bytes produced so far.
    pub fn can_reference(&self, distance: u32) -> bool {
        distance != 0 && distance as usize <= self.filled
    }

    /// Appends a literal byte to the window and `output`.
    pub fn push_literal(&mut self, byte: u8, output: &mut Vec<u8>) {
        self.buffer[self.pos] = byte;
        self.pos = (self.pos + 1) % WINDOW_SIZE;
        if self.filled < WINDOW_SIZE {
            self.filled += 1;
        }
        output.push(byte);
    }

    /// Copies `length` bytes from `distance` bytes back into the window and `output`.
    ///
    /// Callers must first confirm [`SlidingWindow::can_reference`] so that the
    /// decoder owns the bit position reported for invalid references.
    pub fn copy_back(&mut self, distance: u32, length: u32, output: &mut Vec<u8>) {
        for _ in 0..length {
            let src = (self.pos + WINDOW_SIZE - distance as usize) % WINDOW_SIZE;
            let byte = self.buffer[src];
            self.buffer[self.pos] = byte;
            self.pos = (self.pos + 1) % WINDOW_SIZE;
            if self.filled < WINDOW_SIZE {
                self.filled += 1;
            }
            output.push(byte);
        }
    }
}

impl Default for SlidingWindow {
    fn default() -> Self {
        Self::new()
    }
}
