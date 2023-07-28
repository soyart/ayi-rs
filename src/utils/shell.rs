use std::process::Command;

use crate::errors::AyiError;

pub fn exec(cmd: &str, args: &[&str]) -> Result<(), AyiError> {
    match Command::new(cmd).args(args).spawn() {
        Ok(mut result) => match result.wait() {
            Ok(r) => r.exit_ok().map_err(|err| {
                return AyiError::CmdFailed(
                    None,
                    format!("command {cmd} exited with bad status {}", err.to_string()),
                );
            }),
            Err(err) => Err(AyiError::CmdFailed(
                Some(err),
                format!("command ${cmd} failed to run"),
            )),
        },
        Err(err) => Err(AyiError::CmdFailed(
            Some(err),
            format!("command ${cmd} failed to spawn"),
        )),
    }
}

#[test]
fn test_exec() {
    exec("ls", &["-a", "-l"]).expect("failed to execute `ls -a -l` command");
    exec("ls", &["-al"]).expect("failed to execute `ls -al` command");
}
