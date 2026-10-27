//! Allocation-free kernel state and cooperative, non-blocking command polling.

use crate::terminal::{HyperSpaceTerminal, VECTOR_BYTES};

const CONFIG_TEXT_CAPACITY: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigError {
    InvalidRootToken,
    ValueTooLong,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PollResult {
    Idle,
    Processed,
    Dropped,
}

#[derive(Clone, Copy)]
struct FixedText {
    bytes: [u8; CONFIG_TEXT_CAPACITY],
    length: u8,
}

impl FixedText {
    fn from_str(value: &str) -> Self {
        let mut text = Self {
            bytes: [0; CONFIG_TEXT_CAPACITY],
            length: 0,
        };
        let _ = text.set(value);
        text
    }

    fn set(&mut self, value: &str) -> Result<(), ConfigError> {
        if value.len() > CONFIG_TEXT_CAPACITY {
            return Err(ConfigError::ValueTooLong);
        }
        self.bytes = [0; CONFIG_TEXT_CAPACITY];
        self.bytes[..value.len()].copy_from_slice(value.as_bytes());
        self.length = value.len() as u8;
        Ok(())
    }

    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.length as usize]).unwrap_or("")
    }
}

struct ProtectedConfig {
    operator: FixedText,
    sys_version: FixedText,
}

/// Kernel configuration is private and may only be changed through token-gated methods.
pub struct HyperKernel {
    config: ProtectedConfig,
}

impl HyperKernel {
    pub fn new() -> Self {
        Self {
            config: ProtectedConfig {
                operator: FixedText::from_str("Mateusz Mitek"),
                sys_version: FixedText::from_str("1.0.0-HyperOS"),
            },
        }
    }

    pub fn operator(&self) -> &str {
        self.config.operator.as_str()
    }

    pub fn sys_version(&self) -> &str {
        self.config.sys_version.as_str()
    }

    pub fn set_operator(
        &mut self,
        terminal: &HyperSpaceTerminal,
        token: &[u8; VECTOR_BYTES],
        value: &str,
    ) -> Result<(), ConfigError> {
        if value.len() > CONFIG_TEXT_CAPACITY {
            return Err(ConfigError::ValueTooLong);
        }
        if !terminal.verify_root_token(token) {
            return Err(ConfigError::InvalidRootToken);
        }
        self.config.operator.set(value)
    }

    pub fn set_sys_version(
        &mut self,
        terminal: &HyperSpaceTerminal,
        token: &[u8; VECTOR_BYTES],
        value: &str,
    ) -> Result<(), ConfigError> {
        if value.len() > CONFIG_TEXT_CAPACITY {
            return Err(ConfigError::ValueTooLong);
        }
        if !terminal.verify_root_token(token) {
            return Err(ConfigError::InvalidRootToken);
        }
        self.config.sys_version.set(value)
    }

    /// Process at most one command per call; `None` returns immediately as idle.
    pub fn poll(
        &self,
        terminal: &HyperSpaceTerminal,
        command: Option<&str>,
        output: &mut [u8; VECTOR_BYTES],
    ) -> PollResult {
        match command {
            None => {
                *output = [0; VECTOR_BYTES];
                PollResult::Idle
            }
            Some(text) if terminal.process_text_command(text, output) => PollResult::Processed,
            Some(_) => PollResult::Dropped,
        }
    }
}

impl Default for HyperKernel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{ConfigError, HyperKernel, PollResult};
    use crate::terminal::{HyperSpaceTerminal, ROOT_TOKEN_HASH, VECTOR_BYTES};

    #[test]
    fn protected_values_require_the_root_token() {
        let terminal = HyperSpaceTerminal::new();
        let mut kernel = HyperKernel::new();
        assert_eq!(kernel.operator(), "Mateusz Mitek");
        assert_eq!(kernel.sys_version(), "1.0.0-HyperOS");

        assert_eq!(
            kernel.set_operator(&terminal, &[0; VECTOR_BYTES], "Other"),
            Err(ConfigError::InvalidRootToken)
        );
        assert_eq!(kernel.operator(), "Mateusz Mitek");

        assert_eq!(
            kernel.set_operator(&terminal, &ROOT_TOKEN_HASH, "Other"),
            Ok(())
        );
        assert_eq!(kernel.operator(), "Other");
    }

    #[test]
    fn poll_is_single_step_and_non_blocking() {
        let terminal = HyperSpaceTerminal::new();
        let kernel = HyperKernel::new();
        let mut output = [0xff; VECTOR_BYTES];
        assert_eq!(kernel.poll(&terminal, None, &mut output), PollResult::Idle);
        assert_eq!(output, [0; VECTOR_BYTES]);
    }
}