//! Path A: capture → native video sink (research R4).

fn main() -> anyhow::Result<()> {
    println!("{}", capture::init()?);
    Ok(())
}
