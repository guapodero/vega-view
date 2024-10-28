use clap::Parser;
use std::{
    borrow::Cow,
    fmt::Debug,
    fs::File,
    io::{stdin, Read},
    path::{Path, PathBuf},
    time::Instant,
};
use tao::{
    dpi::PhysicalSize,
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use wry::{
    http::{Method, Request, Response, StatusCode},
    WebViewBuilder,
};

const SCHEME: &str = "view";
const BASE: &str = "view://local/page";
const PAGE: &[u8] = include_bytes!("vega-page.html");
const SCRIPT: &[u8] = include_bytes!("vega-all.js");

/// Display a Web View, usually for Vega visualizations.
#[derive(Parser, Clone, Debug)]
struct Args {
    /// A vega-lite specification for this visualization.
    spec: String,

    /// A file containing a HTML template for the page.
    #[arg(long)]
    page: Option<PathBuf>,

    /// A file containing javascript used in the page.
    #[arg(long)]
    script: Option<PathBuf>,

    /// A file containing data to visualize (default is stdin).
    #[arg(long)]
    data: Option<PathBuf>,

    /// The window title.
    #[arg(long)]
    title: Option<String>,

    /// The window width.
    #[arg(long)]
    width: Option<u32>,

    /// The window height.
    #[arg(long)]
    height: Option<u32>,

    /// Turn on debug logging.
    #[arg(long)]
    debug: bool,
}

fn main() -> wry::Result<()> {
    let args = Args::parse();
    let log = Log::new(args.debug);
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title(args.title.as_deref().unwrap_or("Vega View"))
        .with_inner_size(PhysicalSize::new(
            args.width.unwrap_or(1000),
            args.height.unwrap_or(800),
        ))
        .with_decorations(true)
        .build(&event_loop)
        .unwrap();

    let webview_builder = if cfg!(not(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "ios",
        target_os = "android",
    ))) {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;
        let vbox = window.default_vbox().unwrap();
        WebViewBuilder::new_gtk(vbox)
    } else {
        WebViewBuilder::new(&window)
    };

    let _webview = webview_builder
        .with_custom_protocol(SCHEME.to_string(), move |r| handler(log, &args, r))
        .with_url(BASE)
        .with_devtools(true)
        .build()?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(StartCause::WaitCancelled { .. }) => {}
            Event::MainEventsCleared => {}
            Event::RedrawEventsCleared => {}
            Event::DeviceEvent { .. } => {}
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
                log.print(&event)
            }
            _ => log.print(&event),
        }
    });
}

/// Respond to a local http request.
fn handler(log: Log, args: &Args, request: Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    log.print(&request);
    match *request.method() {
        Method::GET => match request.uri().path() {
            "/page" => {
                let body = if let Some(path) = &args.page {
                    Cow::from(file_contents(path.as_path()))
                } else {
                    Cow::from(PAGE)
                };
                Response::builder()
                    .header("Content-Type", "text/html")
                    .body(body)
                    .unwrap()
            }
            "/script" => {
                let body = if let Some(path) = &args.script {
                    Cow::from(file_contents(path.as_path()))
                } else {
                    Cow::from(SCRIPT)
                };
                Response::builder()
                    .header("Content-Type", "text/javascript")
                    .body(body)
                    .unwrap()
            }
            "/spec" => {
                let body = Cow::from(args.spec.clone().into_bytes());
                Response::builder()
                    .header("Content-Type", "application/json")
                    .body(body)
                    .unwrap()
            }
            "/data" => {
                let body = if let Some(path) = &args.data {
                    Cow::from(file_contents(path.as_path()))
                } else {
                    Cow::from(all_input())
                };
                log.print(format!("Data Length {}", body.len()));
                Response::builder()
                    .header("Content-Type", "application/json")
                    .body(body)
                    .unwrap()
            }
            _ => Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Cow::from("Not found".as_bytes()))
                .unwrap(),
        },
        _ => Response::builder()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .body(Cow::from("Wrong method".as_bytes()))
            .unwrap(),
    }
}

/// All the bytes from stdin.
fn all_input() -> Vec<u8> {
    let mut buf = Vec::<u8>::new();
    let _n = stdin().read_to_end(&mut buf).expect("unable to read stdin");
    buf
}

/// All the bytes in a file.
fn file_contents(path: &Path) -> Vec<u8> {
    let mut handle = File::open(path).expect("file not found");
    let mut buf = Vec::<u8>::new();
    handle.read_to_end(&mut buf).expect("unable to read file");
    buf
}

/// A pimitive logger with millisecond timestamps.
#[derive(Debug, Clone, Copy)]
enum Log {
    Enabled(Instant),
    Disabled,
}

impl Log {
    fn new(enabled: bool) -> Self {
        if enabled {
            Self::Enabled(Instant::now())
        } else {
            Self::Disabled
        }
    }
    fn print(self, item: impl Debug) {
        match self {
            Self::Enabled(start) => eprintln!("{} {item:?}", start.elapsed().as_millis()),
            Self::Disabled => {}
        }
    }
}
