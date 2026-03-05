/// CLI configuration and session definition parsing.
///
/// Sessions are defined as positional arguments in "command@path" format.
/// Environment overrides apply to all sessions (SEC-006).

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "tuix",
    version,
    about = "Terminal session multiplexer TUI",
    after_help = "\
EXAMPLES:
    tuix bash                         Single bash session
    tuix bash zsh                     Two sessions side by side
    tuix claude@~/project             Run claude in a specific directory
    tuix bash@/tmp zsh@~/src          Multiple sessions with directories
    tuix bash --env TERM=xterm-256color   Override environment variable

KEYBINDINGS:
    Tile view:  Arrow keys / Tab   Navigate tiles
                Enter / Click      Focus a session
                0-9                Focus by index
                Ctrl+b             Toggle blur
                Ctrl+h             Toggle help screen
                Ctrl+q             Quit
    Focus view: Ctrl+]             Return to tile view
                Click [X]          Return to tile view
                All other input    Forwarded to session"
)]
pub struct Config {
    /// Session definitions: "command" or "command@path"
    #[arg(required = true, value_name = "SESSION")]
    pub sessions: Vec<String>,

    /// Environment variable overrides for all sessions (KEY=VALUE).
    /// Child processes inherit the parent environment by default.
    /// Use this flag to override or add variables.
    #[arg(long = "env", value_name = "KEY=VALUE", value_parser = parse_env_pair)]
    pub env_overrides: Vec<(String, String)>,
}

/// A parsed session definition ready for spawning.
#[derive(Debug)]
pub struct SessionDef {
    pub command: String,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
}

/// Parse a "KEY=VALUE" string into a tuple.
fn parse_env_pair(s: &str) -> Result<(String, String), String> {
    let pos = s
        .find('=')
        .ok_or_else(|| format!("invalid env format '{s}', expected KEY=VALUE"))?;
    Ok((s[..pos].to_string(), s[pos + 1..].to_string()))
}

/// Parse a session definition string ("command@path" or "command").
fn parse_session_def(s: &str, env: &[(String, String)]) -> Result<SessionDef, String> {
    let (command, cwd) = if let Some(pos) = s.rfind('@') {
        let cmd = &s[..pos];
        let path = &s[pos + 1..];
        if cmd.is_empty() {
            return Err(format!("empty command in session definition '{s}'"));
        }
        let resolved = if path.starts_with('/') {
            PathBuf::from(path)
        } else {
            std::env::current_dir()
                .map_err(|e| format!("cannot resolve cwd: {e}"))?
                .join(path)
        };
        (cmd.to_string(), resolved)
    } else {
        let cwd = std::env::current_dir()
            .map_err(|e| format!("cannot get current dir: {e}"))?;
        (s.to_string(), cwd)
    };

    Ok(SessionDef {
        command,
        cwd,
        env: env.to_vec(),
    })
}

/// Parse all session definitions from the config.
pub fn parse_session_defs(config: &Config) -> Result<Vec<SessionDef>, String> {
    config
        .sessions
        .iter()
        .map(|s| parse_session_def(s, &config.env_overrides))
        .collect()
}

/// Parse and validate all session definitions before TUI launch.
/// Checks that commands exist in PATH and directories exist.
pub fn validate(config: &Config) -> Result<Vec<SessionDef>, String> {
    let defs = parse_session_defs(config)?;
    for def in &defs {
        if !def.cwd.is_dir() {
            return Err(format!(
                "directory '{}' does not exist",
                def.cwd.display()
            ));
        }
        if !def.command.contains('/') {
            let path_var = std::env::var("PATH").unwrap_or_default();
            let found = path_var
                .split(':')
                .filter(|d| !d.is_empty())
                .any(|d| std::path::Path::new(d).join(&def.command).exists());
            if !found {
                return Err(format!(
                    "command '{}' not found in PATH",
                    def.command
                ));
            }
        }
    }
    Ok(defs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_command_only() {
        let def = parse_session_def("bash", &[]).unwrap();
        assert_eq!(def.command, "bash");
        assert_eq!(def.cwd, std::env::current_dir().unwrap());
    }

    #[test]
    fn parse_command_at_path() {
        let def = parse_session_def("claude@/tmp", &[]).unwrap();
        assert_eq!(def.command, "claude");
        assert_eq!(def.cwd, PathBuf::from("/tmp"));
    }

    #[test]
    fn parse_env_pair_valid() {
        let (k, v) = parse_env_pair("FOO=bar").unwrap();
        assert_eq!(k, "FOO");
        assert_eq!(v, "bar");
    }

    #[test]
    fn parse_env_pair_invalid() {
        assert!(parse_env_pair("NOEQUALS").is_err());
    }

    #[test]
    fn parse_empty_command_rejected() {
        assert!(parse_session_def("@/tmp", &[]).is_err());
    }

    #[test]
    fn env_overrides_propagated() {
        let env = vec![("KEY".to_string(), "VAL".to_string())];
        let def = parse_session_def("bash", &env).unwrap();
        assert_eq!(def.env.len(), 1);
        assert_eq!(def.env[0].0, "KEY");
    }

    #[test]
    fn validate_rejects_bad_command() {
        let config = Config {
            sessions: vec!["tuix_nonexistent_cmd_xyz".to_string()],
            env_overrides: vec![],
        };
        let err = validate(&config).unwrap_err();
        assert!(err.contains("not found in PATH"), "got: {err}");
    }

    #[test]
    fn validate_rejects_bad_directory() {
        let config = Config {
            sessions: vec!["bash@/no/such/dir/tuix_test".to_string()],
            env_overrides: vec![],
        };
        let err = validate(&config).unwrap_err();
        assert!(err.contains("does not exist"), "got: {err}");
    }

    #[test]
    fn validate_accepts_valid_session() {
        // "sh" exists on all Unix systems
        let config = Config {
            sessions: vec!["sh".to_string()],
            env_overrides: vec![],
        };
        assert!(validate(&config).is_ok());
    }
}
