pub mod providers;
pub mod tester;

use crate::error::LeakError;

pub async fn test(full: bool) -> Result<(), LeakError> {
    let report = tester::run_with(full).await?;
    tester::print(&report);
    Ok(())
}