use std::{
    borrow::Cow,
    collections::HashMap,
    env::{self, JoinPathsError},
    ffi::{OsStr, OsString},
    fmt::Debug,
    iter,
    path::Path,
    process::Command,
};

use tracing::{error, instrument, trace};

use crate::{io::file::Hierarchy, metadata::game::VersionInfo};

fn substitute_arg(arg: &str, params: &HashMap<&str, Cow<'_, OsStr>>) -> OsString {
    if let Some(i) = arg.find("${") {
        if let Some(j) = arg[i..].find('}') {
            if let Some(replacement) = params.get(&arg[i + 2..i + j]) {
                let mut output = OsString::new();
                output.push(OsStr::new(&arg[..i]));
                output.push(replacement);
                output.push(OsStr::new(&arg[i + j + 1..]));
                return output;
            }
        }
    }
    OsString::from(arg)
}

#[derive(Debug)]
pub struct GameCommand<'a> {
    pub cwd: &'a Path,
    pub jvm_args: Vec<OsString>,
    pub game_args: Vec<OsString>,
    pub main_class: &'a str,
}

impl<'a> GameCommand<'a> {
    fn build_classpath(
        version: &VersionInfo,
        hierarchy: &Hierarchy,
    ) -> Result<OsString, JoinPathsError> {
        env::join_paths(
            version
                .libraries
                .iter()
                .filter_map(|lib| {
                    if lib.is_supported_by_rules() {
                        lib.resources.artifact.as_ref()
                    } else {
                        None
                    }
                })
                .map(|artifact| hierarchy.libraries_dir.join(&artifact.path))
                .chain(iter::once(hierarchy.version_dir.join("client.jar"))),
        )
    }

    #[instrument(level = "trace")]
    pub fn from_version_info(
        hierarchy: &'a Hierarchy,
        version: &'a VersionInfo,
        features: &HashMap<&str, bool>,
        username: &str,
    ) -> Self {
        const LAUNCHER_NAME: &str = env!("CARGO_PKG_NAME");
        const LAUNCHER_VERSION: &str = env!("CARGO_PKG_VERSION");

        let mut params = HashMap::new();
        params.insert("launcher_name", Cow::Borrowed(LAUNCHER_NAME.as_ref()));
        params.insert("launcher_version", Cow::Borrowed(LAUNCHER_VERSION.as_ref()));

        params.insert(
            "natives_directory",
            Cow::Borrowed(hierarchy.natives_dir.as_os_str()),
        );
        params.insert(
            "game_directory",
            Cow::Borrowed(hierarchy.gamedir.as_os_str()),
        );
        params.insert(
            "assets_root",
            Cow::Borrowed(hierarchy.assets_dir.as_os_str()),
        );

        match Self::build_classpath(version, hierarchy) {
            Ok(classpath) => {
                trace!(?classpath, "Built classpath");
                params.insert("classpath", Cow::Owned(classpath));
            }
            Err(e) => {
                error!(%e, "Error appending classpath to params");
            }
        }

        params.insert("version_name", Cow::Borrowed(version.id.as_ref()));
        params.insert("assets_index_name", Cow::Borrowed(version.assets.as_ref()));
        params.insert("auth_player_name", Cow::Borrowed(username.as_ref()));
        // TODO : and so on

        trace!(?params, "Gather params for substitution");

        let jvm_args = version
            .arguments
            .iter_jvm_args(&features)
            .map(|arg| substitute_arg(arg, &params))
            .collect();
        let game_args = version
            .arguments
            .iter_game_args(&features)
            .map(|arg| substitute_arg(arg, &params))
            .collect();
        trace!(?jvm_args, "Compiled jvm_args");
        trace!(?game_args, "Compiled game_args");

        Self {
            cwd: hierarchy.gamedir.as_path(),
            main_class: &version.main_class,
            jvm_args,
            game_args,
        }
    }

    #[instrument]
    pub fn build(&self, java_path: impl AsRef<OsStr> + Debug) -> Command {
        let mut command = Command::new(java_path);
        command.current_dir(self.cwd);
        command.args(&self.jvm_args);
        command.arg(OsStr::new(&self.main_class));
        command.args(&self.game_args);
        command
    }
}
