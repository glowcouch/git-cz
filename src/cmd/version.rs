use std::fmt;

use semver::{BuildMetadata, Prerelease, Version};

use crate::{
    cli::VersionCommand,
    cmd::Command,
    conventional::{CommitParser, Config, Type},
    git::{GitHelper, VersionAndTag},
    Error,
};

enum Label {
    /// Bump minor version (0.1.0 -> 1.0.0)
    Major,
    /// Bump minor version (0.1.0 -> 0.2.0)
    Minor,
    /// Bump the patch field (0.1.0 -> 0.1.1)
    Patch,
    /// Remove the pre-release extension; if any (0.1.0-dev.1 -> 0.1.0, 0.1.0 -> 0.1.0)
    Release,
}

impl fmt::Display for Label {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Major => write!(f, "major"),
            Self::Minor => write!(f, "minor"),
            Self::Patch => write!(f, "patch"),
            Self::Release => write!(f, "release"),
        }
    }
}

// helper functions for migrate from old semver crate
// ref to https://github.com/dtolnay/semver/issues/243#issuecomment-854337640
fn increment_patch(v: &mut Version) {
    v.patch += 1;
    v.pre = Prerelease::EMPTY;
    v.build = BuildMetadata::EMPTY;
}

fn increment_minor(v: &mut Version) {
    v.minor += 1;
    v.patch = 0;
    v.pre = Prerelease::EMPTY;
    v.build = BuildMetadata::EMPTY;
}

fn increment_major(v: &mut Version) {
    v.major += 1;
    v.minor = 0;
    v.patch = 0;
    v.pre = Prerelease::EMPTY;
    v.build = BuildMetadata::EMPTY;
}

impl VersionCommand {
    /// returns the versions under the given rev
    fn find_last_version(&self) -> Result<Option<VersionAndTag>, Error> {
        let prefix = self.prefix.as_str();
        Ok(GitHelper::new(prefix)?.find_last_version(self.rev.as_str())?)
    }

    /// Find the bump version based on the conventional commit types.
    ///
    /// - `fix` type commits are translated to PATCH releases.
    /// - `feat` type commits are translated to MINOR releases.
    /// - Commits with `BREAKING CHANGE` in the commits, regardless of type, are translated to MAJOR releases.
    ///
    /// If the project is in major version zero (0.y.z) the rules are:
    ///
    /// - `fix` type commits are translated to PATCH releases.
    /// - `feat` type commits are translated to PATCH releases.
    /// - Commits with `BREAKING CHANGE` in the commits, regardless of type, are translated to MINOR releases.
    fn find_bump_version(
        &self,
        last_v_tag: &str,
        mut last_version: Version,
        parser: &CommitParser,
    ) -> Result<(Version, Label), Error> {
        let prefix = self.prefix.as_str();
        let git = GitHelper::new(prefix)?;
        let mut revwalk = git.revwalk()?;
        revwalk.push_range(format!("{}..{}", last_v_tag, self.rev).as_str())?;
        let i = revwalk
            .flatten()
            .filter_map(|oid| git.find_commit(oid).ok())
            .filter_map(|commit| commit.message().and_then(|msg| parser.parse(msg).ok()));

        let mut major = false;
        let mut minor = false;
        let mut patch = false;

        let major_version_zero = last_version.major == 0;

        for commit in i {
            if commit.breaking {
                if major_version_zero {
                    minor = true;
                } else {
                    major = true;
                }
                break;
            }
            match (commit.r#type, major_version_zero) {
                (Type::Feat, true) => patch = true,
                (Type::Feat, false) => minor = true,
                (Type::Fix, _) => patch = true,
                _ => (),
            }
        }
        let label = match (major, minor, patch) {
            (true, _, _) => {
                increment_major(&mut last_version);
                Label::Major
            }
            (false, true, _) => {
                increment_minor(&mut last_version);
                Label::Minor
            }
            (false, false, true) => {
                increment_patch(&mut last_version);
                Label::Patch
            }
            // TODO what should be the behaviour? always increment patch? or stay on same version?
            _ => Label::Release,
        };
        Ok((last_version, label))
    }
}

impl Command for VersionCommand {
    fn exec(&self, config: Config) -> Result<(), Error> {
        if let Some(VersionAndTag { tag, mut version }) = self.find_last_version()? {
            let v = if self.major {
                increment_major(&mut version);
                (version, Label::Major)
            } else if self.minor {
                increment_minor(&mut version);
                (version, Label::Minor)
            } else if self.patch {
                increment_patch(&mut version);
                (version, Label::Patch)
            } else if self.bump {
                if !version.pre.is_empty() {
                    version.pre = Prerelease::EMPTY;
                    version.build = BuildMetadata::EMPTY;
                    (version, Label::Release)
                } else {
                    let parser = CommitParser::builder()
                        .scope_regex(config.scope_regex)
                        .build();
                    self.find_bump_version(tag.as_str(), version, &parser)?
                }
            } else {
                (version, Label::Release)
            };
            if self.label {
                println!("{}", v.1);
            } else {
                println!("{}", v.0);
            }
        } else {
            println!("0.1.0");
        }
        Ok(())
    }
}
