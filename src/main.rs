use clap::Parser;
// use tokio::net::TcpStream;
// use tokio::io::{AsyncWriteExt as _, self};
use reqwest;

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
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let http_methods = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS", "CONNECT", "TRACE"];
    let args = Args::parse();

    if !http_methods.contains(&args.method.as_str()) {
        panic!("Method not valid")
    }

    let mut client = reqwest::ClientBuilder::new();
    client = client.redirect(reqwest::redirect::Policy::none());
    let clientready = client.build().unwrap();
    let mut req = clientready.request(reqwest::Method::from_bytes(args.method.as_bytes()).unwrap(), args.url.as_str());

    for header in args.header {
        let splitheader: Vec<&str> = header.split(": ").collect();
        if splitheader.len() != 2 {
            continue
        }
        req = req.header(splitheader[0], splitheader[1]);
    }

    if let Some(body) = args.data {
        req = req.body(body);
    }

    let resp = req.send().await?;

    let status = resp.status();
    let text = resp.text().await?;

    println!("Status code: {}", status);
    println!("{:#}", text);

    Ok(())
}
