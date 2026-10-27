//! Native per-user configuration discovery and first-run initialization.
use crate::{
    cli::Mode,
    config::{self, Config, ConfigError},
};
use std::{
    io,
    path::{Component, Path, PathBuf},
};

// Build-time input, never a runtime file lookup or a second set of decoder defaults.
const EXAMPLE: &[u8] = include_bytes!("../../../config.example.json");

#[derive(Clone)]
pub(crate) struct Locations {
    config: PathBuf,
    data: PathBuf,
    logs: PathBuf,
}

fn environment_directory(name: &'static str) -> Result<PathBuf, ConfigError> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .ok_or_else(|| {
            ConfigError::Invalid(format!(
                "{name} must identify an absolute directory; use --config for an explicit installation"
            ))
        })
}

#[cfg(target_os = "linux")]
fn xdg(name: &'static str, fallback: &str) -> Result<PathBuf, ConfigError> {
    // XDG specifies that empty or relative values are ignored.
    if let Some(path) = std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
    {
        return Ok(path.join("ruphish"));
    }
    Ok(environment_directory("HOME")?
        .join(fallback)
        .join("ruphish"))
}

impl Locations {
    fn discover() -> Result<Self, ConfigError> {
        #[cfg(windows)]
        {
            let root = environment_directory("LOCALAPPDATA")?.join("ruphish");
            Ok(Self {
                config: root.join("config.json"),
                data: root.join("data"),
                logs: root.join("logs"),
            })
        }
        #[cfg(target_os = "linux")]
        {
            Ok(Self {
                config: xdg("XDG_CONFIG_HOME", ".config")?.join("config.json"),
                data: xdg("XDG_DATA_HOME", ".local/share")?,
                logs: xdg("XDG_STATE_HOME", ".local/state")?,
            })
        }
        #[cfg(target_os = "macos")]
        {
            let root = environment_directory("HOME")?.join("Library");
            let data = root.join("Application Support/ruphish");
            Ok(Self {
                config: data.join("config.json"),
                data,
                logs: root.join("Logs/ruphish"),
            })
        }
        #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
        {
            Err(ConfigError::Invalid(
                "Default installation directories are unsupported on this OS; use --config".into(),
            ))
        }
    }

    fn initialize(&self) -> Result<(), ConfigError> {
        let bytes = self.defaults()?;
        for directory in [
            self.config.parent(),
            Some(self.data.as_path()),
            Some(self.logs.as_path()),
        ]
        .into_iter()
        .flatten()
        {
            let mut builder = std::fs::DirBuilder::new();
            builder.recursive(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            builder.create(directory)?;
        }
        ruphish_adapters::private_file::publish_new(&self.config, &bytes)?;
        Ok(())
    }

    fn defaults(&self) -> Result<Vec<u8>, ConfigError> {
        // Validate the source through the real decoder before transforming only paths.
        config::decode(EXAMPLE)?.validate(Mode::All)?;
        let invalid = || ConfigError::Invalid("Bundled default configuration is invalid".into());
        let mut value: serde_json::Value =
            serde_json::from_slice(EXAMPLE).map_err(|_| invalid())?;
        for (pointer, directory) in [
            ("/db_path", &self.data),
            ("/admin_server/cert_path", &self.data),
            ("/admin_server/key_path", &self.data),
            ("/phish_server/cert_path", &self.data),
            ("/phish_server/key_path", &self.data),
            ("/logging/filename", &self.logs),
        ] {
            let field = value.pointer_mut(pointer).ok_or_else(invalid)?;
            let relative = Path::new(field.as_str().ok_or_else(invalid)?);
            if relative.as_os_str().is_empty()
                || !relative
                    .components()
                    .all(|part| matches!(part, Component::Normal(_)))
            {
                return Err(invalid());
            }
            *field = serde_json::to_value(directory.join(relative)).map_err(|_| {
                ConfigError::Invalid(
                    "Default configuration paths must be representable as Unicode".into(),
                )
            })?;
        }
        let mut bytes = serde_json::to_vec_pretty(&value).map_err(|_| invalid())?;
        bytes.push(b'\n');
        config::decode(&bytes)?.validate(Mode::All)?;
        Ok(bytes)
    }
}

pub(crate) fn load(explicit: Option<&Path>) -> Result<Config, ConfigError> {
    if let Some(path) = explicit {
        eprintln!("Configuration: {:?}", std::path::absolute(path)?);
        return config::load(path);
    }
    load_default(&Locations::discover()?)
}

fn load_default(locations: &Locations) -> Result<Config, ConfigError> {
    eprintln!("Configuration: {:?}", locations.config);
    let mut config = match config::load(&locations.config) {
        Ok(config) => config,
        Err(ConfigError::Io(error)) if error.kind() == io::ErrorKind::NotFound => {
            locations.initialize().map_err(|error| match error {
                ConfigError::Io(error) => ConfigError::Initialize(error),
                error => error,
            })?;
            config::load(&locations.config)?
        }
        Err(error) => return Err(error),
    };
    if let Some(parent) = locations.config.parent() {
        config.resolve_paths(parent)?;
    }
    if config.db_name == "sqlite3" {
        eprintln!("SQLite database: {:?}", config.db_path);
    }
    if !config.logging.filename.as_os_str().is_empty() {
        eprintln!("Application log: {:?}", config.logging.filename);
    }
    Ok(config)
}

#[cfg(test)]
#[path = "../../../tests/startup_installation.rs"]
mod tests;
