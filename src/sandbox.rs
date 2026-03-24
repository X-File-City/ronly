#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "linux")]
pub use linux::run;

#[cfg(not(target_os = "linux"))]
pub fn run(_args: crate::Args) -> anyhow::Result<()> {
    anyhow::bail!("ronly only runs on Linux")
}
