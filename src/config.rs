/// Configuration file format for floki
use crate::errors;
use crate::image;
use anyhow::Error;
use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;
use std::fs::File;
use std::path;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum Shell {
    Shell(String),
    TwoShell { inner: String, outer: String },
}

impl Shell {
    pub(crate) fn inner_shell(&self) -> &str {
        match self {
            Shell::Shell(s) => s,
            Shell::TwoShell { inner: s, outer: _ } => s,
        }
    }

    pub(crate) fn outer_shell(&self) -> &str {
        match self {
            Shell::Shell(s) => s,
            Shell::TwoShell { inner: _, outer: s } => s,
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum DindConfig {
    Toggle(bool),
    Image { image: String },
}

impl DindConfig {
    pub fn deactivated() -> Self {
        DindConfig::Toggle(false)
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
/// The Volume structure captures configuration for floki volumes
pub(crate) struct Volume {
    #[serde(default = "default_to_false")]
    /// A shared volume is reused by containers which also use a
    /// shared volume by the same name. Volumes which are not
    /// shared are localised to a particular floki configuration file.
    pub(crate) shared: bool,
    /// The mount path is the path at which the volume is mounted
    /// inside the floki container.
    pub(crate) mount: path::PathBuf,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum Entrypoint {
    Suppress { suppress: bool },
}

impl Entrypoint {
    pub fn value(&self) -> Option<&str> {
        match self {
            Entrypoint::Suppress { suppress } if *suppress => Some(""),
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FlokiConfig {
    pub(crate) image: image::Image,
    #[serde(default = "Vec::new")]
    pub(crate) init: Vec<String>,
    #[serde(default = "default_shell")]
    pub(crate) shell: Shell,
    #[serde(default = "default_mount")]
    pub(crate) mount: path::PathBuf,
    #[serde(default = "Vec::new")]
    pub(crate) docker_switches: Vec<String>,
    #[serde(default = "default_to_false")]
    pub(crate) forward_ssh_agent: bool,
    #[serde(default = "DindConfig::deactivated")]
    pub(crate) dind: DindConfig,
    #[serde(default = "default_to_false")]
    pub(crate) forward_user: bool,
    #[serde(default = "BTreeMap::new")]
    pub(crate) volumes: BTreeMap<String, Volume>,
    #[serde(default = "default_entrypoint")]
    pub(crate) entrypoint: Entrypoint,
}

impl FlokiConfig {
    pub fn from_file(file: &path::Path) -> Result<FlokiConfig, Error> {
        debug!("Reading configuration file: {:?}", file);

        let f = File::open(file).map_err(|e| errors::FlokiError::ProblemOpeningConfigYaml {
            name: file.display().to_string(),
            error: e,
        })?;

        let mut config: FlokiConfig = serde_yaml::from_reader(f).map_err(|e| {
            errors::FlokiError::ProblemParsingConfigYaml {
                name: file.display().to_string(),
                error: e,
            }
        })?;

        // Ensure the path to an external yaml file is correct.
        // If the image.yaml.path file is relative, then it should
        // be relative to the floki config file. At this point we
        // already have the path to the floki config file, so we
        // just prepend that to image.yaml.path.
        if let image::Image::Yaml { ref mut yaml } = config.image {
            if let image::YamlSpec::File { file, .. } = yaml {
                if file.is_relative() {
                    *file = file
                        .parent()
                        .ok_or_else(|| errors::FlokiInternalError::InternalAssertionFailed {
                            description: format!(
                                "could not construct path to external yaml file '{:?}'",
                                &file
                            ),
                        })?
                        .join(file.clone());
                }
            }
        }

        debug!(
            "Parsed '{}' into configuration: {:?}",
            file.display(),
            &config
        );

        Ok(config)
    }
}

fn default_shell() -> Shell {
    Shell::Shell("sh".into())
}

fn default_mount() -> path::PathBuf {
    path::Path::new("/src").to_path_buf()
}

fn default_to_false() -> bool {
    false
}

fn default_entrypoint() -> Entrypoint {
    Entrypoint::Suppress { suppress: true }
}

#[cfg(test)]
mod test {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct TestShellConfig {
        shell: Shell,
    }

    #[test]
    fn test_single_shell_config() {
        let yaml = "shell: bash";
        let expected = TestShellConfig {
            shell: Shell::Shell("bash".into()),
        };
        let actual: TestShellConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(actual == expected);
    }

    #[test]
    fn test_two_shell_config() {
        let yaml = "shell:\n  outer: sh\n  inner: bash";
        let expected_shell = Shell::TwoShell {
            inner: "bash".into(),
            outer: "sh".into(),
        };
        let expected = TestShellConfig {
            shell: expected_shell,
        };
        let actual: TestShellConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(actual == expected);
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct TestDindConfig {
        dind: DindConfig,
    }

    #[test]
    fn test_dind_enabled_config() {
        let yaml = "dind: true";
        let expected = TestDindConfig {
            dind: DindConfig::Toggle(true),
        };
        let actual: TestDindConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_dind_image_config() {
        let yaml = "dind:\n  image: dind:custom";
        let expected = TestDindConfig {
            dind: DindConfig::Image {
                image: "dind:custom".into(),
            },
        };
        let actual: TestDindConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(actual, expected);
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct TestEntrypointConfig {
        entrypoint: Entrypoint,
    }

    #[test]
    fn test_entrypoint_suppress() {
        let yaml = "entrypoint:\n  suppress: true";
        let expected = TestEntrypointConfig {
            entrypoint: Entrypoint::Suppress { suppress: true },
        };
        let actual: TestEntrypointConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(actual.entrypoint.value(), Some(""));
    }

    #[test]
    fn test_entrypoint_no_suppress() {
        let yaml = "entrypoint:\n  suppress: false";
        let expected = TestEntrypointConfig {
            entrypoint: Entrypoint::Suppress { suppress: false },
        };
        let actual: TestEntrypointConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(actual.entrypoint.value(), None);
    }
}
