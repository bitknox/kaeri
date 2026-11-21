//! `kaeri --plugins`: the plugins installed here, and what each one may reach. And the
//! arguments of `kaeri plugin …`, which runs beside the store's commands in `store`.

use std::time::Duration;

use anyhow::Result;
use kaeri_core::config::{Config, Paths};

#[derive(clap::Subcommand, Debug)]
pub enum PluginCommand {
    /// Write a plugin's manifest into it, so a store that cannot run plugins can read it.
    Stamp(kaeri_store_server::StampArgs),
}

/// List installed plugins and what each may reach.
///
/// Loaded with **no HTTP client**, so inspecting a plugin cannot cause it to make a request. A
/// plugin describing itself is the one call that runs with no capabilities at all.
///
/// Fails after the list when a plugin did not load, so a script can tell. A book source does
/// not count: nothing is wrong with it, it is just not a show source.
pub async fn list_plugins(config: &Config, paths: &Paths) -> Result<()> {
    let dir = paths.plugin_dir();
    println!("plugin directory  {}", dir.display());

    let limits = kaeri_plugin::Limits {
        memory_bytes: config.providers.plugins.memory_mb.max(1).saturating_mul(1024 * 1024),
        deadline: Duration::from_secs(config.providers.plugins.deadline_secs.max(1)),
        ..Default::default()
    };
    println!(
        // MiB, as the config documents it. The number is the file's own, so it keeps its unit:
        // "64 MB" was a decimal name on what is 67 MB.
        "limits            {} MiB memory · {} s per call",
        config.providers.plugins.memory_mb,
        limits.deadline.as_secs()
    );
    if !config.providers.order.iter().any(|p| p == "plugins") {
        println!(
            "note              \"plugins\" is not in providers.order, so none will be used"
        );
    }
    println!();

    let host = kaeri_plugin::PluginHost::new(limits, None)?;
    let loaded = host.load_dir(&dir).await;
    if loaded.is_empty() {
        println!("No plugins installed.");
        println!();
        println!(
            "Drop a WebAssembly component into that directory. To build the reference one:"
        );
        println!(
            "  cargo build --release --target wasm32-wasip2 \\\n    \
             --manifest-path plugins/example-rust/Cargo.toml"
        );
        return Ok(());
    }

    let total = loaded.len();
    let mut failed = 0;
    for result in loaded {
        match result {
            Ok(plugin) => {
                let manifest = plugin.manifest();
                println!("● {}  {}", manifest.id, manifest.version);
                println!("  name          {}", manifest.display_name);
                println!("  translations  {}", manifest.translation_types.join(", "));
                // The line that matters: everything this plugin can reach, enforced host-side.
                if manifest.allowed_hosts.is_empty() {
                    println!("  may contact    nothing (declares no hosts)");
                } else {
                    println!("  may contact   {}", manifest.allowed_hosts.join(", "));
                }
                println!("  cannot        read files, open sockets, read your environment");
            }
            Err(e) => {
                println!("✕ {e}");
                if !matches!(e, kaeri_plugin::PluginError::OtherKind { .. }) {
                    failed += 1;
                }
            }
        }
        println!();
    }
    if failed > 0 {
        anyhow::bail!("{failed} of {total} plugins did not load");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_plugin_that_does_not_load_fails_the_listing() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let paths = Paths {
            config_file: dir.path().join("config.toml"),
            data_dir: dir.path().join("data"),
            cache_dir: dir.path().join("cache"),
        };
        std::fs::create_dir_all(paths.plugin_dir()).expect("a plugin directory");
        std::fs::write(paths.plugin_dir().join("broken.wasm"), b"not a component")
            .expect("a broken plugin");

        let listed = list_plugins(&Config::default(), &paths).await;
        let error = listed.expect_err("a plugin that did not load must fail the listing");
        assert_eq!(error.to_string(), "1 of 1 plugins did not load");
    }
}
