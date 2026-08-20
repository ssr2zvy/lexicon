pub trait HttpAcquisition {
    fn run(&self) -> Result<(), String>;
}

pub fn run_http_source<A>(acquisition: A) -> Result<(), String>
where
    A: HttpAcquisition,
{
    acquisition.run()
}
