//! JSON-lines harness for integration tests and headless development.
//! The desktop app calls this same Service directly; it never spawns this binary.
use rust_hmcl::{
    service::{Job, Service},
    Config,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
#[derive(Deserialize)]
#[serde(tag = "method", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    Snapshot,
    Task,
    Versions,
    Runtimes,
    ModFiles,
    ResourcePacks,
    CancelJob,
    UpdateConfig {
        config: Box<Config>,
    },
    StartJob {
        job: Job,
    },
    ToggleMod {
        name: String,
    },
    EnableResources {
        names: Vec<String>,
        enabled: bool,
        confirmed: bool,
    },
}
fn execute(service: &Service, request: Request) -> anyhow::Result<Value> {
    Ok(match request {
        Request::Snapshot => serde_json::to_value(service.snapshot()?)?,
        Request::Task => serde_json::to_value(service.task()?)?,
        Request::Versions => serde_json::to_value(service.versions()?)?,
        Request::Runtimes => serde_json::to_value(service.runtimes()?)?,
        Request::ModFiles => serde_json::to_value(service.mod_files()?)?,
        Request::ResourcePacks => serde_json::to_value(service.resource_packs()?)?,
        Request::CancelJob => {
            service.cancel();
            Value::Null
        }
        Request::UpdateConfig { config } => serde_json::to_value(service.update_config(*config)?)?,
        Request::StartJob { job } => json!(service.start(job)?),
        Request::ToggleMod { name } => serde_json::to_value(service.toggle_mod(&name)?)?,
        Request::EnableResources {
            names,
            enabled,
            confirmed,
        } => serde_json::to_value(service.enable_resources(&names, enabled, confirmed)?)?,
    })
}
fn main() -> anyhow::Result<()> {
    let service = Service::load()?;
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    for line in stdin.lock().lines() {
        let response = match serde_json::from_str::<Request>(&line?)
            .map_err(anyhow::Error::from)
            .and_then(|request| execute(&service, request))
        {
            Ok(value) => json!({"ok":true,"value":value}),
            Err(error) => json!({"ok":false,"error":format!("{error:#}")}),
        };
        writeln!(stdout, "{response}")?;
        stdout.flush()?;
    }
    Ok(())
}
