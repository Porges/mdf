use std::fmt::Display;

use crate::{RawLine, Sink};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum GEDCOMVersion {
    V5,
    V7,
}

impl Display for GEDCOMVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GEDCOMVersion::V5 => write!(f, "5.5.1"),
            GEDCOMVersion::V7 => write!(f, "7.0"),
        }
    }
}

#[derive(thiserror::Error, Debug)]
#[error("invalid GEDCOM version")]
pub struct InvalidGEDCOMVersionError {}

pub fn parse_gedcom_version_raw(value: &[u8]) -> Result<GEDCOMVersion, InvalidGEDCOMVersionError> {
    match value {
        b"5.5.1" => Ok(GEDCOMVersion::V5),
        b"7.0" => Ok(GEDCOMVersion::V7),
        _ => Err(InvalidGEDCOMVersionError {}),
    }
}

pub struct VersionErrorAdapter<ETarget, Inner> {
    inner: Inner,
    _phantom: std::marker::PhantomData<ETarget>,
}

impl<ETarget, Inner> VersionErrorAdapter<ETarget, Inner> {
    pub fn new(inner: Inner) -> Self {
        Self {
            inner,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<'a, Inner, ETarget> Sink<RawLine<'a>> for VersionErrorAdapter<ETarget, Inner>
where
    Inner: Sink<RawLine<'a>>,
    ETarget: From<Inner::Err>,
{
    type Err = ETarget;
    type Output = Inner::Output;

    fn consume(&mut self, item: RawLine<'a>) -> Result<(), Self::Err> {
        self.inner.consume(item)?;
        Ok(())
    }

    fn complete(self) -> Result<Self::Output, Self::Err> {
        Ok(self.inner.complete()?)
    }
}

impl GEDCOMVersion {
    pub fn get_format_handler<
        'a,
        E: From<crate::v5::RecordError> + From<crate::v7::RecordError> + 'static,
    >(
        self,
    ) -> Box<dyn Sink<RawLine<'a>, Output = (), Err = E>> {
        match self {
            GEDCOMVersion::V5 => Box::new(VersionErrorAdapter::new(crate::v5::RecordParser {})),
            GEDCOMVersion::V7 => Box::new(VersionErrorAdapter::new(crate::v7::RecordParser {})),
        }
    }
}
