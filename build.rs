use std::{path::Path, process::Command};

const SCRIPT_CMD: &str = "./scripts/build.sh";

fn main() -> Result<(), std::io::Error> {
    let mut cmd = Command::new(SCRIPT_CMD);
    cmd.current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("contracts"));
    let mut child = cmd.spawn()?;
    let _ = child.wait()?;
    Ok(())
}
