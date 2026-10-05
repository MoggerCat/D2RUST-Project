//! Little-endian cursor over a byte slice, shared by the file-format parsers.

/// A format parser rejected its input.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FormatError {
    #[error("{format}: data ends early (needed {needed} bytes at offset {offset:#x})")]
    Truncated {
        format: &'static str,
        offset: usize,
        needed: usize,
    },
    #[error("{format}: {message}")]
    Invalid {
        format: &'static str,
        message: String,
    },
}

pub(crate) fn invalid(format: &'static str, message: impl Into<String>) -> FormatError {
    FormatError::Invalid {
        format,
        message: message.into(),
    }
}

pub(crate) struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
    format: &'static str,
}

impl<'a> Cursor<'a> {
    pub(crate) fn new(data: &'a [u8], format: &'static str) -> Self {
        Self {
            data,
            pos: 0,
            format,
        }
    }

    pub(crate) fn at(data: &'a [u8], pos: usize, format: &'static str) -> Self {
        Self { data, pos, format }
    }

    pub(crate) fn pos(&self) -> usize {
        self.pos
    }

    pub(crate) fn bytes(&mut self, n: usize) -> Result<&'a [u8], FormatError> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.data.len());
        match end {
            Some(end) => {
                let s = &self.data[self.pos..end];
                self.pos = end;
                Ok(s)
            }
            None => Err(FormatError::Truncated {
                format: self.format,
                offset: self.pos,
                needed: n,
            }),
        }
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], FormatError> {
        let b = self.bytes(N)?;
        let mut a = [0u8; N];
        a.copy_from_slice(b);
        Ok(a)
    }

    pub(crate) fn u8(&mut self) -> Result<u8, FormatError> {
        Ok(self.array::<1>()?[0])
    }

    pub(crate) fn u16(&mut self) -> Result<u16, FormatError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    pub(crate) fn u32(&mut self) -> Result<u32, FormatError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    pub(crate) fn i32(&mut self) -> Result<i32, FormatError> {
        Ok(i32::from_le_bytes(self.array()?))
    }
}
