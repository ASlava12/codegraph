//! Optional desktop launcher: a native WebView shell that starts the
//! embedded CodeGraph server and opens the web UI for a local repository.

use anyhow::{Context, Result, anyhow, bail};
use clap::Parser;
use std::env;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop};
use tao::window::WindowBuilder;
use wry::WebViewBuilder;

#[derive(Debug, Parser)]
#[command(name = "codegraph-ui")]
#[command(about = "Run CodeGraph as a local desktop application")]
#[command(version)]
struct Args {
    /// Project root exposed to the scanner.
    #[arg(long, default_value = ".")]
    root: PathBuf,

    /// Additional local project roots available in the project selector.
    #[arg(long = "project")]
    projects: Vec<PathBuf>,

    /// HTTP bind host for the embedded local backend.
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// HTTP bind port for the embedded backend. Use 0 to auto-select a local port.
    #[arg(long, default_value_t = 0)]
    port: u16,

    /// Attach to an already running CodeGraph server instead of spawning one.
    #[arg(long)]
    server_url: Option<String>,

    /// Path to the codegraph-server binary. Also read from CODEGRAPH_SERVER_BIN.
    #[arg(long)]
    server_bin: Option<PathBuf>,

    /// Include hidden files and directories in scans.
    #[arg(long)]
    include_hidden: bool,

    /// Include default ignored directories such as target and node_modules.
    #[arg(long)]
    include_ignored: bool,

    /// Maximum bytes to read from any single file during scans.
    #[arg(long)]
    max_file_size: Option<u64>,

    /// Allow scanning paths outside the configured root.
    #[arg(long)]
    allow_any_path: bool,

    /// Disable persistent graph cache.
    #[arg(long)]
    no_cache: bool,

    /// Directory for persistent graph cache records.
    #[arg(long)]
    cache_dir: Option<PathBuf>,

    /// Require a token for local API routes. Also read by codegraph-server from CODEGRAPH_API_TOKEN.
    #[arg(long)]
    api_token: Option<String>,

    /// Desktop window width in logical pixels.
    #[arg(long, default_value_t = 1440.0)]
    window_width: f64,

    /// Desktop window height in logical pixels.
    #[arg(long, default_value_t = 940.0)]
    window_height: f64,
}

enum ServerLaunch {
    Binary(PathBuf),
    Cargo { workspace_root: PathBuf },
}

fn main() -> Result<()> {
    let args = Args::parse();
    let env_api_token = env::var("CODEGRAPH_API_TOKEN").ok();
    let health_api_token = args.api_token.as_deref().or(env_api_token.as_deref());
    let (url, server_child) = if let Some(server_url) = args.server_url.as_deref() {
        (normalize_server_url(server_url), None)
    } else {
        let port = if args.port == 0 {
            available_local_port(&args.host)?
        } else {
            args.port
        };
        let url = format!("http://{}:{port}", args.host);
        let mut child = spawn_server(&args, port)?;
        if let Err(error) = wait_for_health(&args.host, port, health_api_token) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error)
                .with_context(|| format!("CodeGraph backend did not become ready at {url}"));
        }
        (url, Some(child))
    };

    run_window(&args, &url, server_child)
}

fn run_window(args: &Args, url: &str, mut server_child: Option<Child>) -> Result<()> {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("CodeGraph")
        .with_inner_size(LogicalSize::new(args.window_width, args.window_height))
        .build(&event_loop)
        .context("failed to create CodeGraph desktop window")?;
    let _webview = WebViewBuilder::new()
        .with_url(url)
        .build(&window)
        .with_context(|| format!("failed to create webview for {url}"))?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            if let Some(child) = server_child.as_mut() {
                let _ = child.kill();
                let _ = child.wait();
            }
            *control_flow = ControlFlow::Exit;
        }
    });
}

fn spawn_server(args: &Args, port: u16) -> Result<Child> {
    let launch = resolve_server_launch(args.server_bin.as_deref())?;
    let mut command = match launch {
        ServerLaunch::Binary(path) => Command::new(path),
        ServerLaunch::Cargo { workspace_root } => {
            let mut command = Command::new("cargo");
            command.current_dir(workspace_root);
            command.args(["run", "-p", "codegraph-server", "--"]);
            command
        }
    };
    command
        .arg("--root")
        .arg(&args.root)
        .arg("--host")
        .arg(&args.host)
        .arg("--port")
        .arg(port.to_string())
        .arg("--quiet-access-log");
    for project in &args.projects {
        command.arg("--project").arg(project);
    }
    if args.include_hidden {
        command.arg("--include-hidden");
    }
    if args.include_ignored {
        command.arg("--include-ignored");
    }
    if let Some(max_file_size) = args.max_file_size {
        command
            .arg("--max-file-size")
            .arg(max_file_size.to_string());
    }
    if args.allow_any_path {
        command.arg("--allow-any-path");
    }
    if args.no_cache {
        command.arg("--no-cache");
    }
    if let Some(cache_dir) = &args.cache_dir {
        command.arg("--cache-dir").arg(cache_dir);
    }
    if let Some(api_token) = &args.api_token {
        command.arg("--api-token").arg(api_token);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("failed to start codegraph-server backend")
}

fn resolve_server_launch(explicit: Option<&Path>) -> Result<ServerLaunch> {
    if let Some(path) = explicit {
        return Ok(ServerLaunch::Binary(path.to_path_buf()));
    }
    if let Some(path) = env::var_os("CODEGRAPH_SERVER_BIN").map(PathBuf::from) {
        return Ok(ServerLaunch::Binary(path));
    }
    if let Some(path) = sibling_server_binary()? {
        return Ok(ServerLaunch::Binary(path));
    }
    if let Some(workspace_root) = find_workspace_root()? {
        return Ok(ServerLaunch::Cargo { workspace_root });
    }
    bail!("could not find codegraph-server; pass --server-bin or set CODEGRAPH_SERVER_BIN");
}

fn sibling_server_binary() -> Result<Option<PathBuf>> {
    let current_exe = env::current_exe().context("failed to locate current executable")?;
    let Some(dir) = current_exe.parent() else {
        return Ok(None);
    };
    let candidate = dir.join(server_binary_name());
    Ok(candidate.is_file().then_some(candidate))
}

fn server_binary_name() -> &'static str {
    if cfg!(windows) {
        "codegraph-server.exe"
    } else {
        "codegraph-server"
    }
}

fn find_workspace_root() -> Result<Option<PathBuf>> {
    let mut dir = env::current_dir().context("failed to read current directory")?;
    loop {
        let manifest = dir.join("Cargo.toml");
        if manifest.is_file() {
            let content = std::fs::read_to_string(&manifest)
                .with_context(|| format!("failed to read {}", manifest.display()))?;
            if content.contains("[workspace]") && content.contains("codegraph-server") {
                return Ok(Some(dir));
            }
        }
        if !dir.pop() {
            return Ok(None);
        }
    }
}

fn available_local_port(host: &str) -> Result<u16> {
    let listener = std::net::TcpListener::bind((host, 0))
        .with_context(|| format!("failed to reserve a local port on {host}"))?;
    let port = listener
        .local_addr()
        .context("failed to read reserved local address")?
        .port();
    Ok(port)
}

fn wait_for_health(host: &str, port: u16, api_token: Option<&str>) -> Result<()> {
    let started = Instant::now();
    let timeout = Duration::from_secs(30);
    let address = format!("{host}:{port}");
    while started.elapsed() < timeout {
        if health_request(&address, api_token).is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    Err(anyhow!("timed out waiting for /api/health on {address}"))
}

fn health_request(address: &str, api_token: Option<&str>) -> Result<()> {
    let mut stream = TcpStream::connect(address)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let auth_header = api_token
        .map(|token| format!("authorization: Bearer {token}\r\n"))
        .unwrap_or_default();
    write!(
        stream,
        "GET /api/health HTTP/1.1\r\nHost: {address}\r\n{auth_header}Connection: close\r\n\r\n"
    )?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    if response.starts_with("HTTP/1.1 200") || response.starts_with("HTTP/1.0 200") {
        Ok(())
    } else {
        Err(anyhow!("health check returned non-200 response"))
    }
}

fn normalize_server_url(url: &str) -> String {
    url.trim_end_matches('/').to_string()
}
