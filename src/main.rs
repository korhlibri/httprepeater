use clap::Parser;
use http_body_util::Empty;
use hyper::Request;
use hyper::body::Bytes;
use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;
use http_body_util::BodyExt;
use tokio::io::{AsyncWriteExt as _, self};

/// Make an HTTP request and receive data characteristics
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// What HTTP method to use.
    /// 
    /// Example: -m "POST"
    #[arg(short, long, default_value = "GET")]
    method: String,
    
    /// Header to include in the HTTP request.
    /// Multiple headers may be included, but they must come with their own individual flag.
    /// 
    /// Example: -H "Accept: application/json"
    #[arg(short = 'H', long)]
    header: Vec<String>,

    /// Data to include with the HTTP request.
    /// 
    /// Example: -d '{"username":"john","password":"123456"}'
    #[arg(short, long, required = false)]
    data: Option<String>,

    /// URL to make the request to.
    /// 
    /// Example: -u "http://example.com"
    #[arg(short, long)]
    url: String,

    /// Port to make the request to.
    /// 
    /// Example: -d '{"username":"john","password":"123456"}'
    #[arg(short, long, default_value_t = 80)]
    port: u16,
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let http_methods = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS", "CONNECT", "TRACE"];
    let args = Args::parse();

    // if !http_methods.contains(&args.method.as_str()) {
    //     return Err(String::from("Method not valid"));
    // }
    let url = args.url.parse::<hyper::Uri>()?;
    let host = url.host().expect("No host found in URL");
    let port = args.port;

    let address = format!("{}:{}", host, port);

    let stream = TcpStream::connect(address).await?;

    let io = TokioIo::new(stream);

    let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await?;

    tokio::task::spawn(async move {
        if let Err(err) = conn.await {
            println!("Connection failed: {:?}", err);
        }
    });

    let authority = url.authority().unwrap().clone();

    let req = Request::builder()
        .uri(url)
        .header(hyper::header::HOST, authority.as_str())
        .body(Empty::<Bytes>::new())?;

    let mut res = sender.send_request(req).await?;

    println!("Response status: {}", res.status());

    while let Some(next) = res.frame().await {
        let frame = next?;
        if let Some(chunk) = frame.data_ref() {
            io::stdout().write_all(chunk).await?;
        }
    }


    Ok(())
}
