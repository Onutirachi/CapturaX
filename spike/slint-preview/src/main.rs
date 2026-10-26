//! Path B (video inside Slint) and the visual directions (research R5, R10).

slint::include_modules!();

fn main() -> anyhow::Result<()> {
    println!("{}", capture::init()?);
    App::new()?.run()?;
    Ok(())
}
