use crate::app::output::Output;
use crate::app::release::{self, UpdateStatus, Version};
use crate::app::render;
use crate::app::runtime::Runtime;
use crate::cli::UpgradeArgs;
use anyhow::{Context, Result, bail, ensure};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::Stdio;

const USE_PACKAGE_MANAGER: &str = "upgrade it with the package manager that installed it, or reinstall with the official installer";

pub async fn upgrade(rt: &Runtime, args: &UpgradeArgs) -> Result<Output> {
    let installed = Version::installed()?;
    let target = if let Some(version) = args.version {
        version
    } else {
        let status = UpdateStatus {
            installed,
            latest: release::latest().await?,
        };
        if args.check || !status.update_available() {
            return Ok(Output::view(status.to_json(), render::upgrade));
        }
        status.latest
    };

    let exe = std::env::current_exe().context("could not locate the running kite binary")?;
    let install_dir = install_dir(&exe, invoked_path().as_deref())?;
    if cfg!(windows) {
        bail!(windows_instructions(target, &install_dir));
    }
    let installer_url = release::installer_url(target, false);
    let plan = format!(
        "Upgrading kite {installed} to {target}\n  binary:    {}\n  installer: {installer_url}",
        exe.display()
    );
    if rt.json {
        eprintln!("{plan}");
    } else {
        println!("{plan}");
    }

    let workdir = PrivateDir::create()?;
    let script = release::download_installer(&installer_url, &workdir.0).await?;
    let mut command = tokio::process::Command::new("sh");
    command
        .arg(&script)
        .env("KITE_VERSION", target.to_string())
        .env("KITE_INSTALL_DIR", &install_dir);
    if rt.json {
        // Keep stdout a single JSON document.
        command.stdout(Stdio::from(std::io::stderr()));
    }
    let status = command
        .status()
        .await
        .context("could not run the installer with sh")?;
    ensure!(status.success(), "the installer failed ({status})");

    Ok(Output::view(
        json!({
            "upgraded": true,
            "from": installed.to_string(),
            "to": target.to_string(),
            "install_dir": install_dir,
        }),
        render::upgrade,
    ))
}

/// The `kite` path the shell ran, which may be a symlink to `current_exe`.
fn invoked_path() -> Option<PathBuf> {
    let arg0 = PathBuf::from(std::env::args_os().next()?);
    if arg0.components().count() > 1 {
        return Some(arg0);
    }
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join(&arg0))
        .find(|path| path.symlink_metadata().is_ok())
}

/// Returns the directory the installer may replace `kite` in, or why it must not.
fn install_dir(exe: &Path, invoked: Option<&Path>) -> Result<PathBuf> {
    let paths = [Some(exe), invoked];
    for path in paths.into_iter().flatten() {
        let canonical = path.canonicalize().ok();
        if [Some(path), canonical.as_deref()]
            .into_iter()
            .flatten()
            .any(|path| path.starts_with("/nix/store"))
        {
            bail!(
                "{} is installed from the Nix store; {USE_PACKAGE_MANAGER}",
                path.display()
            );
        }
        if path
            .symlink_metadata()
            .is_ok_and(|meta| meta.file_type().is_symlink())
        {
            bail!(
                "{} is a symlink, which usually means a package manager installed it; {USE_PACKAGE_MANAGER}",
                path.display()
            );
        }
    }
    let binary = if cfg!(windows) { "kite.exe" } else { "kite" };
    let Some(dir) = exe
        .parent()
        .filter(|_| exe.file_name().is_some_and(|name| name == binary))
    else {
        bail!(
            "{} is not named {binary}, so the installer would not replace it; {USE_PACKAGE_MANAGER}",
            exe.display()
        );
    };
    let probe = dir.join(format!(".kite-upgrade-probe-{}", std::process::id()));
    if let Err(error) = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
    {
        bail!(
            "cannot write to {} ({error}); {USE_PACKAGE_MANAGER}",
            dir.display()
        );
    }
    let _ = std::fs::remove_file(&probe);
    Ok(dir.to_path_buf())
}

/// Windows refuses to overwrite a running executable, and install.ps1 replaces
/// kite.exe in place, so the installer must run after kite exits.
fn windows_instructions(target: Version, install_dir: &Path) -> String {
    let dir = install_dir.display().to_string().replace('\'', "''");
    format!(
        "Windows cannot replace kite.exe while it runs. Close kite, then run this in PowerShell:\n  & ([scriptblock]::Create((irm {}))) -Version {target} -InstallDir '{dir}'",
        release::installer_url(target, true)
    )
}

/// A new directory only this user can read, removed on drop.
struct PrivateDir(PathBuf);

impl PrivateDir {
    fn create() -> Result<Self> {
        let path = std::env::temp_dir().join(format!("kite-upgrade-{}", uuid::Uuid::new_v4()));
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        builder
            .create(&path)
            .with_context(|| format!("could not create {}", path.display()))?;
        Ok(Self(path))
    }
}

impl Drop for PrivateDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("kite-upgrade-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn nix_store_binaries_are_refused() {
        let error = install_dir(Path::new("/nix/store/abc-kite-0.2.1/bin/kite"), None).unwrap_err();
        assert_eq!(
            error.to_string(),
            format!(
                "/nix/store/abc-kite-0.2.1/bin/kite is installed from the Nix store; {USE_PACKAGE_MANAGER}"
            )
        );
        let dir = scratch("nix-invoked");
        let exe = dir.join("kite");
        std::fs::write(&exe, "").unwrap();
        let error = install_dir(&exe, Some(Path::new("/nix/store/abc/bin/kite"))).unwrap_err();
        assert!(
            error
                .to_string()
                .starts_with("/nix/store/abc/bin/kite is installed from the Nix store")
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_binaries_are_refused() {
        let dir = scratch("symlink");
        std::fs::create_dir(dir.join("cellar")).unwrap();
        std::fs::create_dir(dir.join("bin")).unwrap();
        let real = dir.join("cellar").join("kite");
        std::fs::write(&real, "").unwrap();
        let link = dir.join("bin").join("kite");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        for (exe, invoked) in [(&real, Some(link.as_path())), (&link, None)] {
            let error = install_dir(exe, invoked).unwrap_err();
            assert_eq!(
                error.to_string(),
                format!(
                    "{} is a symlink, which usually means a package manager installed it; {USE_PACKAGE_MANAGER}",
                    link.display()
                )
            );
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_plain_writable_kite_binary_is_accepted() {
        let dir = scratch("plain");
        let exe = dir.join(if cfg!(windows) { "kite.exe" } else { "kite" });
        std::fs::write(&exe, "").unwrap();
        assert_eq!(install_dir(&exe, Some(&exe)).unwrap(), dir);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        let renamed = dir.join("kite-dev");
        std::fs::write(&renamed, "").unwrap();
        let error = install_dir(&renamed, None).unwrap_err();
        assert!(error.to_string().contains("is not named kite"), "{error}");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn read_only_directories_are_refused() {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch("read-only");
        let exe = dir.join("kite");
        std::fs::write(&exe, "").unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
        // Root ignores directory permissions, so there is nothing to refuse.
        let privileged = std::fs::write(dir.join("probe"), "").is_ok();
        let result = install_dir(&exe, None);
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        if !privileged {
            let error = result.unwrap_err();
            assert!(
                error
                    .to_string()
                    .starts_with(&format!("cannot write to {} (", dir.display())),
                "{error}"
            );
        }
    }

    #[test]
    fn windows_instructions_pin_the_tag_and_install_dir() {
        let text = windows_instructions(
            Version::parse("0.2.2").unwrap(),
            Path::new("C:/Users/o'neil/kite"),
        );
        assert!(text.ends_with(
            "irm https://raw.githubusercontent.com/zerodha/kite-cli/v0.2.2/scripts/install.ps1))) -Version 0.2.2 -InstallDir 'C:/Users/o''neil/kite'"
        ), "{text}");
    }

    #[test]
    fn private_dirs_are_removed_on_drop() {
        let dir = PrivateDir::create().unwrap();
        let path = dir.0.clone();
        assert!(path.is_dir());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
        drop(dir);
        assert!(!path.exists());
    }
}
