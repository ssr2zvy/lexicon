Understood. Based on this report, the immediate next step is to correct the HTTP contract. It has reverted to:

fn run(&self) -> Result<(), String>;

Change it to:

pub struct HttpAcquisitionContext;
pub trait HttpAcquisition {
    fn acquire(
        &self,
        context: &mut HttpAcquisitionContext,
    ) -> Result<(), String>;
}

Then run_http_source should construct the context and call:

acquisition.acquire(&mut context)

Update the generated main.rs template to implement acquire.

Because generated crates use Git tag v0.1.0, do not move that existing tag. Release the corrected Core contract under a new tag and update the generated dependency to that tag.

Success criterion: a newly initialized external project and generated HTTP source both pass cargo check using the context-based contract. After that, the next behavioral step is giving HttpAcquisitionContext its first real operation for making and recording an HTTP request.
