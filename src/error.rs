use derive_more::From;

#[derive(Debug, From, PartialEq)]
pub enum Error<'a> {
    #[cfg(not(feature = "alloc"))]
    /// The destination buffer is too small
    BufferTooSmall,
    /// The provided slice is too short
    SliceTooShort,
    /// The type name format is not supported yet (to be implemented)
    UnsupportedTypeNameFormat(crate::TypeNameFormat),
    /// The provided external type does not contain a ':'
    InvalidExternalType(&'a str),
    /// The record type is not supported yet (to be implemented)
    UnsupportedRecordType(&'a str),
    /// The provided data is not valid UTF-8
    #[from]
    UTF8(core::str::Utf8Error),
    /// The provided data is not valid UTF-8
    #[from]
    #[cfg(feature = "alloc")]
    UTF8Decode(alloc::string::FromUtf8Error),
    /// The provided data is not valid UTF-16
    #[cfg(feature = "alloc")]
    UTF16Decode,
    /// The provided data is odd length
    #[cfg(feature = "alloc")]
    UTF16OddLength(usize),
}

pub type Result<'a, T> = core::result::Result<T, Error<'a>>;

// can be enabled when MSRV >= 1.81
// impl<'a> core::error::Error for Error<'a> {}

impl<'a> core::fmt::Display for Error<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}
