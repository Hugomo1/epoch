pub mod gpu;
pub mod process;
pub mod system;
pub mod training;

use color_eyre::Result;

#[allow(async_fn_in_trait)]
pub trait Collector {
    async fn collect(&mut self) -> Result<()>;
}
