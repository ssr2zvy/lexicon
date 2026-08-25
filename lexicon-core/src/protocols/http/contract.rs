use std::ffi::OsString;

use crate::HttpAcquisitionContext;

use super::error::{AcquisitionError, AcquisitionResult};

pub type HttpAcquireFn = fn(&mut HttpAcquisitionContext, &[OsString]) -> AcquisitionResult<()>;
pub type HttpResumeFn = fn(&mut HttpAcquisitionContext, &[OsString]) -> AcquisitionResult<()>;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum HttpCapability {
    ClientCertificateV1,
    ResumeV1,
}

#[derive(Copy, Clone, Debug)]
pub struct HttpSourceContractV1 {
    acquire: HttpAcquireFn,
    resume: Option<HttpResumeFn>,
    capabilities: [HttpCapability; 8],
    capability_count: usize,
}

impl HttpSourceContractV1 {
    pub const fn new(acquire: HttpAcquireFn) -> Self {
        Self {
            acquire,
            resume: None,
            capabilities: [HttpCapability::ClientCertificateV1; 8],
            capability_count: 0,
        }
    }

    pub const fn with_resume(mut self, resume: HttpResumeFn) -> Self {
        self.resume = Some(resume);
        self
    }

    pub const fn requires(mut self, capability: HttpCapability) -> Self {
        let index = self.capability_count;
        self.capabilities[index] = capability;
        self.capability_count += 1;
        self
    }

    pub const fn acquire_fn(&self) -> HttpAcquireFn {
        self.acquire
    }

    pub const fn resume_fn(&self) -> Option<HttpResumeFn> {
        self.resume
    }

    pub fn capabilities(&self) -> &[HttpCapability] {
        &self.capabilities[..self.capability_count]
    }

    pub fn acquire(
        &self,
        context: &mut HttpAcquisitionContext,
        args: &[OsString],
    ) -> AcquisitionResult<()> {
        (self.acquire)(context, args)
    }

    pub fn resume(
        &self,
        context: &mut HttpAcquisitionContext,
        args: &[OsString],
    ) -> AcquisitionResult<()> {
        match self.resume {
            Some(resume) => resume(context, args),
            None => Err(AcquisitionError::source("resume is not configured for this source")),
        }
    }
}

impl From<HttpAcquireFn> for HttpSourceContractV1 {
    fn from(acquire: HttpAcquireFn) -> Self {
        Self::new(acquire)
    }
}
