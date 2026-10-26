//! Low-latency audio passthrough (research R7).

fn main() -> anyhow::Result<()> {
    println!("{}", capture::init()?);
    Ok(())
}
