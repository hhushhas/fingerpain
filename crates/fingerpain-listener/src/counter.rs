//! Word and paragraph boundary detection

use crate::KeyEventType;

/// Tracks keystrokes to detect word boundaries
pub struct KeystrokeCounter {
    /// Characters typed since last word boundary
    pub pending_chars: u32,
    /// Total characters in current session
    pub total_chars: u32,
    /// Total words in current session
    pub total_words: u32,
    /// Total paragraphs in current session
    pub total_paragraphs: u32,
    /// Total backspaces in current session
    pub total_backspaces: u32,
}

impl KeystrokeCounter {
    pub fn new() -> Self {
        Self {
            pending_chars: 0,
            total_chars: 0,
            total_words: 0,
            total_paragraphs: 0,
            total_backspaces: 0,
        }
    }

    /// Process a key event
    pub fn process(&mut self, event_type: KeyEventType) {
        match event_type {
            KeyEventType::Character => {
                self.pending_chars += 1;
                self.total_chars += 1;
            }
            KeyEventType::Space | KeyEventType::Tab => {
                self.total_chars += 1;
                if self.pending_chars > 0 {
                    self.total_words += 1;
                    self.pending_chars = 0;
                }
            }
            KeyEventType::Enter => {
                self.total_chars += 1;
                self.total_paragraphs += 1;
                if self.pending_chars > 0 {
                    self.total_words += 1;
                    self.pending_chars = 0;
                }
            }
            KeyEventType::Backspace => {
                self.total_backspaces += 1;
                if self.pending_chars > 0 {
                    self.pending_chars -= 1;
                }
            }
            KeyEventType::Other => {}
        }
    }

    /// Reset all counters
    pub fn reset(&mut self) {
        self.pending_chars = 0;
        self.total_chars = 0;
        self.total_words = 0;
        self.total_paragraphs = 0;
        self.total_backspaces = 0;
    }

    /// Get current stats as a tuple (chars, words, paragraphs, backspaces)
    pub fn stats(&self) -> (u32, u32, u32, u32) {
        (
            self.total_chars,
            self.total_words,
            self.total_paragraphs,
            self.total_backspaces,
        )
    }
}

impl Default for KeystrokeCounter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_word_counting() {
        let mut counter = KeystrokeCounter::new();

        // Type "hello world"
        for _ in 0..5 {
            counter.process(KeyEventType::Character); // h, e, l, l, o
        }
        counter.process(KeyEventType::Space);
        for _ in 0..5 {
            counter.process(KeyEventType::Character); // w, o, r, l, d
        }
        counter.process(KeyEventType::Space);

        assert_eq!(counter.total_chars, 12); // 10 letters + 2 spaces
        assert_eq!(counter.total_words, 2);
    }

    #[test]
    fn test_paragraph_counting() {
        let mut counter = KeystrokeCounter::new();

        // Type "line1\nline2\n"
        for _ in 0..5 {
            counter.process(KeyEventType::Character);
        }
        counter.process(KeyEventType::Enter);
        for _ in 0..5 {
            counter.process(KeyEventType::Character);
        }
        counter.process(KeyEventType::Enter);

        assert_eq!(counter.total_paragraphs, 2);
        assert_eq!(counter.total_words, 2);
    }

    #[test]
    fn test_backspace() {
        let mut counter = KeystrokeCounter::new();

        // Type "helo" then backspace and fix to "hello"
        for _ in 0..4 {
            counter.process(KeyEventType::Character);
        }
        counter.process(KeyEventType::Backspace);
        for _ in 0..2 {
            counter.process(KeyEventType::Character);
        }

        assert_eq!(counter.total_chars, 6);
        assert_eq!(counter.total_backspaces, 1);
        assert_eq!(counter.pending_chars, 5); // 4 - 1 + 2
    }
}
