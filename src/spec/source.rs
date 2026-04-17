use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Source {
    Github(GithubSource),
    Local(PathBuf),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubSource {
    pub github: String,
    #[serde(default = "default_ref")]
    pub r#ref: String,
    #[serde(default)]
    pub path: String,
}

fn default_ref() -> String {
    "main".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_path_parses_as_local_variant() {
        let src: Source = serde_yml::from_str("./skills/").unwrap();
        match src {
            Source::Local(p) => assert_eq!(p, PathBuf::from("./skills/")),
            _ => panic!("expected Local"),
        }
    }

    #[test]
    fn github_object_with_defaults() {
        let src: Source = serde_yml::from_str("github: owner/repo\n").unwrap();
        match src {
            Source::Github(g) => {
                assert_eq!(g.github, "owner/repo");
                assert_eq!(g.r#ref, "main");
                assert_eq!(g.path, "");
            }
            _ => panic!("expected Github"),
        }
    }

    #[test]
    fn github_object_with_ref_and_path() {
        let src: Source = serde_yml::from_str(
            "github: owner/repo\nref: v1.0.0\npath: skills/\n",
        )
        .unwrap();
        match src {
            Source::Github(g) => {
                assert_eq!(g.github, "owner/repo");
                assert_eq!(g.r#ref, "v1.0.0");
                assert_eq!(g.path, "skills/");
            }
            _ => panic!("expected Github"),
        }
    }

    #[test]
    fn round_trip_github() {
        let src = Source::Github(GithubSource {
            github: "owner/repo".into(),
            r#ref: "abc123".into(),
            path: "subdir".into(),
        });
        let yaml = serde_yml::to_string(&src).unwrap();
        let back: Source = serde_yml::from_str(&yaml).unwrap();
        match back {
            Source::Github(g) => {
                assert_eq!(g.github, "owner/repo");
                assert_eq!(g.r#ref, "abc123");
                assert_eq!(g.path, "subdir");
            }
            _ => panic!("expected Github"),
        }
    }
}
