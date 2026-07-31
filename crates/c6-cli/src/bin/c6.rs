use std::{ffi::OsString, io::Write};

use c6_cli::{
    AppError, JsonFailure, JsonSuccess, VERSION, authenticated_client,
    config::{Config, Paths, Server},
    credential::{CredentialStore, plaintext_allowed},
    git_credential_config, git_environment, read_secret_stdin, resolve_project, run_git,
    selected_server,
};
use c6_client::{Client, Origin};
use clap::{Args, Parser, Subcommand};
use serde::Serialize;

#[derive(Parser)]
#[command(name="c6", version=VERSION, about="Thin client for a C6 small-software cloud")]
struct Cli {
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Version,
    Server {
        #[command(subcommand)]
        command: ServerCommand,
    },
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    Clone(CloneArgs),
    Remote {
        #[command(subcommand)]
        command: RemoteCommand,
    },
    Doctor(ServerChoice),
}

#[derive(Subcommand)]
enum ServerCommand {
    Add {
        origin: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        allow_http_localhost: bool,
    },
    List,
    Use {
        alias: String,
    },
}
#[derive(Subcommand)]
enum AuthCommand {
    Login {
        #[arg(long)]
        server: Option<String>,
        #[arg(long)]
        token_stdin: bool,
        #[arg(long)]
        plaintext_store: bool,
    },
    Status(ServerChoice),
    Logout(ServerChoice),
}
#[derive(Subcommand)]
enum ProjectCommand {
    List {
        #[arg(long)]
        server: Option<String>,
        #[arg(long)]
        workspace: Option<String>,
    },
}
#[derive(Subcommand)]
enum RemoteCommand {
    Add {
        project: String,
        #[arg(long, default_value = "c6")]
        name: String,
        #[arg(long)]
        server: Option<String>,
    },
}
#[derive(Args)]
struct ServerChoice {
    #[arg(long)]
    server: Option<String>,
}
#[derive(Args)]
struct CloneArgs {
    project: String,
    directory: Option<OsString>,
    #[arg(long)]
    server: Option<String>,
}

fn main() {
    let cli = Cli::parse();
    let json = cli.json;
    match execute(cli) {
        Ok(value) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string(&JsonSuccess::new(value)).expect("JSON serialization")
                );
            }
        }
        Err(error) => {
            if json {
                eprintln!(
                    "{}",
                    serde_json::to_string(&JsonFailure::new(&error)).expect("JSON serialization")
                );
            } else {
                eprintln!("error: {error}");
            }
            std::process::exit(error.exit_code());
        }
    }
}

fn execute(cli: Cli) -> Result<serde_json::Value, AppError> {
    let paths = Paths::discover().map_err(|e| AppError::Local(e.to_string()))?;
    match cli.command {
        Commands::Version => output(
            cli.json,
            &serde_json::json!({"version":VERSION}),
            format!("c6 {VERSION}"),
        ),
        Commands::Server { command } => server_command(&paths, cli.json, command),
        Commands::Auth { command } => auth_command(&paths, cli.json, command),
        Commands::Project { command } => project_command(&paths, cli.json, command),
        Commands::Clone(args) => clone_command(&paths, cli.json, args),
        Commands::Remote { command } => remote_command(&paths, cli.json, command),
        Commands::Doctor(choice) => doctor(&paths, cli.json, choice.server.as_deref()),
    }
}

fn server_command(
    paths: &Paths,
    json: bool,
    command: ServerCommand,
) -> Result<serde_json::Value, AppError> {
    match command {
        ServerCommand::Add {
            origin,
            name,
            allow_http_localhost,
        } => {
            let origin = Origin::parse(&origin, allow_http_localhost)?;
            let status = Client::new(origin.clone(), None)?.status()?;
            let status_server_id = status.server_id.to_string();
            let alias = name.unwrap_or_else(|| {
                status
                    .server_name
                    .to_ascii_lowercase()
                    .chars()
                    .map(|c| {
                        if c.is_ascii_alphanumeric() || c == '-' {
                            c
                        } else {
                            '-'
                        }
                    })
                    .collect::<String>()
                    .trim_matches('-')
                    .to_owned()
            });
            if alias.is_empty()
                || !alias
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                return Err(AppError::Usage(
                    "server alias must contain letters, numbers, '-' or '_'".into(),
                ));
            }
            let _lock = paths.lock().map_err(|e| AppError::Local(e.to_string()))?;
            let mut config = Config::load(paths).map_err(|e| AppError::Local(e.to_string()))?;
            if let Some(existing) = config.servers.get(&alias)
                && (existing.server_id != status_server_id || existing.base_url != origin.as_str())
            {
                return Err(AppError::Conflict);
            }
            config.servers.insert(
                alias.clone(),
                Server {
                    base_url: origin.as_str().into(),
                    server_id: status_server_id.clone(),
                    allow_http_localhost,
                },
            );
            if config.default_server.is_none() {
                config.default_server = Some(alias.clone());
            }
            config
                .save(paths)
                .map_err(|e| AppError::Local(e.to_string()))?;
            output(
                json,
                &serde_json::json!({"name":alias,"baseUrl":origin.as_str(),"serverId":status.server_id}),
                format!("Added {} ({})", alias, origin),
            )
        }
        ServerCommand::List => {
            let config = Config::load(paths).map_err(|e| AppError::Local(e.to_string()))?;
            let values:Vec<_>=config.servers.iter().map(|(name,s)|serde_json::json!({"name":name,"baseUrl":s.base_url,"serverId":s.server_id,"default":config.default_server.as_deref()==Some(name)})).collect();
            if !json {
                for v in &values {
                    println!(
                        "{}\t{}{}",
                        v["name"].as_str().unwrap_or(""),
                        v["baseUrl"].as_str().unwrap_or(""),
                        if v["default"] == true {
                            "\t(default)"
                        } else {
                            ""
                        }
                    );
                }
            }
            Ok(serde_json::json!({"servers":values}))
        }
        ServerCommand::Use { alias } => {
            let _lock = paths.lock().map_err(|e| AppError::Local(e.to_string()))?;
            let mut config = Config::load(paths).map_err(|e| AppError::Local(e.to_string()))?;
            if !config.servers.contains_key(&alias) {
                return Err(AppError::NotFound);
            }
            config.default_server = Some(alias.clone());
            config
                .save(paths)
                .map_err(|e| AppError::Local(e.to_string()))?;
            output(
                json,
                &serde_json::json!({"defaultServer":alias}),
                format!("Using {alias}"),
            )
        }
    }
}

fn auth_command(
    paths: &Paths,
    json: bool,
    command: AuthCommand,
) -> Result<serde_json::Value, AppError> {
    match command {
        AuthCommand::Login {
            server,
            token_stdin,
            plaintext_store,
        } => {
            if !token_stdin {
                return Err(AppError::Usage(
                    "tokens are accepted only with --token-stdin".into(),
                ));
            }
            if !plaintext_allowed(plaintext_store) {
                return Err(AppError::Local("the headless plaintext credential fallback requires --plaintext-store (token will be plaintext at rest)".into()));
            }
            let mut config = Config::load(paths).map_err(|e| AppError::Local(e.to_string()))?;
            let (alias, selected) = selected_server(&config, server.as_deref())?;
            let alias = alias.to_owned();
            let selected_base_url = selected.base_url.clone();
            let origin = Origin::parse(&selected.base_url, selected.allow_http_localhost)?;
            let pinned = selected.server_id.clone();
            let token = read_secret_stdin()?;
            let who = Client::new(origin, Some(token.clone()))?.whoami()?;
            if who.server.id.to_string() != pinned {
                return Err(AppError::Protocol);
            }
            let _lock = paths.lock().map_err(|e| AppError::Local(e.to_string()))?;
            config = Config::load(paths).map_err(|e| AppError::Local(e.to_string()))?;
            let (_, current) = selected_server(&config, Some(&alias))?;
            if current.server_id != pinned || current.base_url != selected_base_url {
                return Err(AppError::Conflict);
            }
            let mut store =
                CredentialStore::load(paths).map_err(|e| AppError::Local(e.to_string()))?;
            store.set_api_token(alias.clone(), token);
            store
                .save(paths)
                .map_err(|e| AppError::Local(e.to_string()))?;
            config.plaintext_credentials = true;
            config
                .save(paths)
                .map_err(|e| AppError::Local(e.to_string()))?;
            if !json {
                eprintln!("warning: credential stored in an owner-only plaintext file");
            }
            output(
                json,
                &serde_json::json!({"server":alias,"user":{"id":who.user.id,"displayName":who.user.display_name}}),
                format!("Logged in to {alias} as {}", who.user.display_name),
            )
        }
        AuthCommand::Status(choice) => {
            let (alias, client) = authenticated_client(paths, choice.server.as_deref())?;
            let who = client.whoami()?;
            output(
                json,
                &serde_json::json!({"server":alias,"user":who.user,"workspaces":who.workspaces}),
                format!("Logged in to {alias} as {}", who.user.display_name),
            )
        }
        AuthCommand::Logout(choice) => {
            let _lock = paths.lock().map_err(|e| AppError::Local(e.to_string()))?;
            let config = Config::load(paths).map_err(|e| AppError::Local(e.to_string()))?;
            let (alias, _) = selected_server(&config, choice.server.as_deref())?;
            let alias = alias.to_owned();
            let mut store =
                CredentialStore::load(paths).map_err(|e| AppError::Local(e.to_string()))?;
            let removed = store.remove_api_token(&alias);
            store
                .save(paths)
                .map_err(|e| AppError::Local(e.to_string()))?;
            output(
                json,
                &serde_json::json!({"server":alias,"removed":removed}),
                format!("Logged out of {alias}"),
            )
        }
    }
}

fn project_command(
    paths: &Paths,
    json: bool,
    command: ProjectCommand,
) -> Result<serde_json::Value, AppError> {
    match command {
        ProjectCommand::List { server, workspace } => {
            let (_, client) = authenticated_client(paths, server.as_deref())?;
            let who = client.whoami()?;
            let mut projects = client.projects()?.projects;
            if let Some(slug) = workspace {
                let id = who
                    .workspaces
                    .iter()
                    .find(|w| w.slug == slug)
                    .ok_or(AppError::NotFound)?
                    .id;
                projects.retain(|p| p.workspace_id == id);
            }
            if !json {
                for p in &projects {
                    let ws = who
                        .workspaces
                        .iter()
                        .find(|w| w.id == p.workspace_id)
                        .map(|w| w.slug.as_str())
                        .unwrap_or("?");
                    println!("{ws}/{}\t{}", p.slug, p.name);
                }
            }
            Ok(serde_json::json!({"projects":projects}))
        }
    }
}

fn clone_command(
    paths: &Paths,
    json: bool,
    args: CloneArgs,
) -> Result<serde_json::Value, AppError> {
    let (_, client) = authenticated_client(paths, args.server.as_deref())?;
    let (_, project) = resolve_project(&client, &args.project)?;
    let remote = client.project_remote(&project.id)?;
    if !remote.capabilities.fetch {
        return Err(AppError::Forbidden);
    }
    let credential = git_credential_config(&remote.clone_url)?;
    let mut argv = vec![
        OsString::from("clone"),
        OsString::from("--config"),
        OsString::from(format!("{}=", credential.helper_key)),
        OsString::from("--config"),
        OsString::from(format!("{}=c6", credential.helper_key)),
        OsString::from("--config"),
        OsString::from(format!("{}=true", credential.use_http_path_key)),
        OsString::from(&remote.clone_url),
    ];
    if let Some(directory) = args.directory {
        argv.push(directory);
    }
    let status = run_git(argv, json)?;
    if !status.success() {
        return Err(AppError::GitFailure);
    }
    output(
        json,
        &serde_json::json!({"projectId":project.id,"cloneUrl":remote.clone_url}),
        "Clone complete".into(),
    )
}

fn remote_command(
    paths: &Paths,
    json: bool,
    command: RemoteCommand,
) -> Result<serde_json::Value, AppError> {
    match command {
        RemoteCommand::Add {
            project,
            name,
            server,
        } => {
            if name.is_empty() || name.starts_with('-') || name.chars().any(char::is_whitespace) {
                return Err(AppError::Usage("invalid Git remote name".into()));
            }
            let (_, client) = authenticated_client(paths, server.as_deref())?;
            let (_, project) = resolve_project(&client, &project)?;
            let remote = client.project_remote(&project.id)?;
            if !remote.capabilities.fetch {
                return Err(AppError::Forbidden);
            }
            let credential = git_credential_config(&remote.clone_url)?;
            let reset_status = run_git(
                [
                    OsString::from("config"),
                    OsString::from("--local"),
                    OsString::from("--replace-all"),
                    OsString::from(&credential.helper_key),
                    OsString::new(),
                ],
                json,
            )?;
            if !reset_status.success() {
                return Err(AppError::GitFailure);
            }
            let helper_status = run_git(
                [
                    OsString::from("config"),
                    OsString::from("--local"),
                    OsString::from("--add"),
                    OsString::from(&credential.helper_key),
                    OsString::from("c6"),
                ],
                json,
            )?;
            if !helper_status.success() {
                return Err(AppError::GitFailure);
            }
            let path_status = run_git(
                [
                    OsString::from("config"),
                    OsString::from("--local"),
                    OsString::from(&credential.use_http_path_key),
                    OsString::from("true"),
                ],
                json,
            )?;
            if !path_status.success() {
                return Err(AppError::GitFailure);
            }
            let status = run_git(
                [
                    OsString::from("remote"),
                    OsString::from("add"),
                    OsString::from(&name),
                    OsString::from(&remote.clone_url),
                ],
                json,
            )?;
            if !status.success() {
                return Err(AppError::GitFailure);
            }
            output(
                json,
                &serde_json::json!({"name":name,"cloneUrl":remote.clone_url}),
                format!("Added remote {name}"),
            )
        }
    }
}

fn doctor(paths: &Paths, json: bool, server: Option<&str>) -> Result<serde_json::Value, AppError> {
    let git = git_environment()?;
    let config = Config::load(paths).map_err(|e| AppError::Local(e.to_string()))?;
    let (alias, selected) = selected_server(&config, server)?;
    let origin = Origin::parse(&selected.base_url, selected.allow_http_localhost)?;
    let status = Client::new(origin, None)?.status()?;
    if status.server_id.to_string() != selected.server_id {
        return Err(AppError::Protocol);
    }
    let store = CredentialStore::load(paths).map_err(|e| AppError::Local(e.to_string()))?;
    let authenticated = store.api_token(alias).is_some();
    let data = serde_json::json!({"git":git,"server":{"name":alias,"baseUrl":selected.base_url,"serverId":status.server_id},"credentialStore":{"kind":"owner_only_plaintext","ready":config.plaintext_credentials,"authenticated":authenticated},"transport":{"https":selected.base_url.starts_with("https://")}});
    if !json {
        println!(
            "Git: {}\nServer: {} ({})\nAuthentication: {}",
            git["version"],
            alias,
            selected.base_url,
            if authenticated {
                "configured"
            } else {
                "missing"
            }
        );
    }
    Ok(data)
}

fn output<T: Serialize>(
    json: bool,
    value: &T,
    human: String,
) -> Result<serde_json::Value, AppError> {
    let value = serde_json::to_value(value).map_err(|e| AppError::Internal(e.to_string()))?;
    if !json {
        println!("{human}");
        std::io::stdout()
            .flush()
            .map_err(|e| AppError::Internal(e.to_string()))?;
    }
    Ok(value)
}
