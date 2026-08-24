//! Version arguments (`minecraftArguments` and structured `arguments`).
//!
//! Port of HMCL's `game.Arguments` / `game.RuledArgument` / `game.StringArgument`.

use serde::{Deserialize, Serialize};

use super::rules::{CompatibilityRule, OperatingSystem};

/// A single argument entry: either a plain string or a ruled argument
/// (a string plus the compatibility rules that guard it).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Argument {
    Plain(String),
    Ruled {
        rules: Vec<CompatibilityRule>,
        value: ArgumentValue,
    },
}

/// The value of a ruled argument (single string or a list).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ArgumentValue {
    Single(String),
    List(Vec<String>),
}

/// The structured `arguments` section of a version manifest.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<Argument>,
    #[serde(default)]
    pub jvm: Vec<Argument>,
}

impl Arguments {
    /// All game arguments applicable to `os`, as individual strings.
    pub fn game_args_for(&self, os: &OperatingSystem) -> Vec<String> {
        flatten_args(&self.game, os)
    }

    /// All JVM arguments applicable to `os`, as individual strings.
    pub fn jvm_args_for(&self, os: &OperatingSystem) -> Vec<String> {
        flatten_args(&self.jvm, os)
    }
}

fn flatten_args(args: &[Argument], os: &OperatingSystem) -> Vec<String> {
    let mut out = Vec::new();
    for argument in args {
        match argument {
            Argument::Plain(value) => out.push(value.clone()),
            Argument::Ruled { rules, value } => {
                if rules.iter().all(|rule| rule.allows(os)) {
                    match value {
                        ArgumentValue::Single(value) => out.push(value.clone()),
                        ArgumentValue::List(values) => out.extend(values.iter().cloned()),
                    }
                }
            }
        }
    }
    out
}

/// A legacy `minecraftArguments` string (pre-1.13 style).
#[derive(Debug, Clone, Default)]
pub struct StringArgument(pub String);

impl StringArgument {
    /// Split a legacy arguments string into tokens on spaces.
    pub fn tokens(&self) -> Vec<String> {
        self.0
            .split(' ')
            .map(str::to_owned)
            .filter(|token| !token.is_empty())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arguments_parse() {
        let json = r#"{
            "game": [
                "--username",
                "${auth_player_name}",
                {"rules": [{"action": "allow", "os": {"name": "osx"}}], "value": ["--osx-only"]},
                {"rules": [{"action": "disallow", "os": {"name": "windows"}}], "value": "--not-windows"}
            ],
            "jvm": ["-Djava.library.path=${natives_directory}"]
        }"#;
        let arguments: Arguments = serde_json::from_str(json).unwrap();
        let game = arguments.game_args_for(&OperatingSystem::Windows);
        assert_eq!(game, vec!["--username", "${auth_player_name}"]);
        let game_osx = arguments.game_args_for(&OperatingSystem::Osx);
        assert_eq!(
            game_osx,
            vec![
                "--username",
                "${auth_player_name}",
                "--osx-only",
                "--not-windows"
            ]
        );
        let jvm = arguments.jvm_args_for(&OperatingSystem::Windows);
        assert_eq!(jvm, vec!["-Djava.library.path=${natives_directory}"]);
    }

    #[test]
    fn test_string_argument_tokens() {
        let legacy =
            StringArgument("--username ${auth_player_name} --version ${version_name}".to_owned());
        assert_eq!(legacy.tokens().len(), 4);
    }
}
