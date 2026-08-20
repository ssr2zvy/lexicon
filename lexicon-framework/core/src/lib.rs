pub struct HttpAcquisitionContext;

pub trait HttpAcquisition {
    fn acquire(
        &self,
        context: &mut HttpAcquisitionContext,
    ) -> Result<(), String>;
}

pub fn run_http_source<A>(acquisition: A) -> Result<(), String>
where
    A: HttpAcquisition,
{
    let mut context = HttpAcquisitionContext;
    acquisition.acquire(&mut context)
}
