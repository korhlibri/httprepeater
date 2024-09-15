use clap::Parser;
// use tokio::net::TcpStream;
// use tokio::io::{AsyncWriteExt as _, self};
use reqwest;
use std::sync::{Arc, Mutex};
use std::fs::File;
use std::io::{self, BufRead};
// use std::path::Path;

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

    /// Body to include with the HTTP request.
    /// 
    /// Example: -b '{"username":"john","password":"123456"}'
    #[arg(short, long, required = false)]
    body: Option<String>,

    /// URL to make the request to.
    /// 
    /// Example: -u "http://example.com"
    #[arg(short, long)]
    url: String,

    /// Wordlist file to use for repeated HTTP requests.
    /// 
    /// Example: -l "words.txt"
    #[arg(short, long)]
    list: String,

    /// Delimiter to change the data between it with each wordlist item.
    /// 
    /// Example: -b '{"username":"john","password":"##123456##"}' -d "##"
    #[arg(short, long)]
    delim: String,

    /// Displays the response headers and body.
    /// 
    /// Example: -b '{"username":"john","password":"##123456##"}' -d "##" --verbose
    #[arg(long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let http_methods = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS", "CONNECT", "TRACE"];
    let args = Args::parse();
    let wordlist = Arc::new(Mutex::new(Vec::<String>::new()));
    
    let filler = Arc::clone(&wordlist);
    // TODO: REPLACE WITH NON-BLOCKING SPAWN
    load_words_to_memory(args.list, filler).await;
    // tokio::spawn(async move {
    //     load_words_to_memory(args.list, filler).await;
    // });

    if !http_methods.contains(&args.method.as_str()) {
        panic!("Method not valid")
    }

    let mut client = reqwest::ClientBuilder::new();
    client = client.redirect(reqwest::redirect::Policy::none());
    let clientready = client.build().unwrap();

    // TODO: IMPLEMENT NON-BLOCKING THREADS
    let mut wordsmutex = wordlist.lock().unwrap();

    let mut headers: Vec<Vec<(String, Vec<(usize, _)>)>> = Vec::new();
    for header in &args.header {
        let splitheader: Vec<&str> = header.split(": ").collect();
        if splitheader.len() != 2 {
            continue
        }
        let mut tempvec: Vec<(String, Vec<(usize, _)>)> = Vec::new();
        for value in splitheader {
            let indices: Vec<(usize, _)> = value.match_indices(args.delim.as_str()).collect();

            if indices.len() % 2 != 0 {
                panic!("Delimiters need to be set in pairs");
            }
            else {
                tempvec.push((value.to_string(), indices));
            }
        }

        headers.push(tempvec);
    }

    let mut bodies: Option<(String, Vec<(usize, _)>)> = None;
    if let Some(body) = &args.body {
        let indices: Vec<(usize, _)> = body.match_indices(args.delim.as_str()).collect();

        if indices.len() % 2 != 0 {
            panic!("Delimiters need to be set in pairs");
        }

        bodies = Some((body.clone(), indices));
    }

    while wordsmutex.len() > 0 {
        let word = match wordsmutex.pop() {
            Some(word) => word,
            None => continue,
        };
        let mut req = clientready.request(reqwest::Method::from_bytes(args.method.as_bytes()).unwrap(), args.url.as_str());

        for header in &headers {
            let mut iterator: usize = 0;
            let mut key: String = String::from("");
            let mut value: String = String::from("");
            if header[0].1.len() > 0 {
                while iterator < header[0].1.len() {
                    let first_delim_pos = header[0].1[iterator].0;
                    let last_delim_pos = {
                        if iterator == 0 {
                            0
                        } else {
                            header[0].1[iterator-1].0+args.delim.len()
                        }
                    };

                    key.push_str(&header[0].0[last_delim_pos..first_delim_pos]);
                    key.push_str(&word);
                    iterator += 2;
                }

                let last_delim_pos = header[0].1[iterator-1].0+args.delim.len();
                if last_delim_pos < header[0].0.len() {
                    key.push_str(&header[0].0[last_delim_pos..])
                }

            } else {
                key.push_str(&header[0].0);
            }
            iterator = 0;
            if header[1].1.len() > 0 {
                while iterator < header[1].1.len() {
                    let first_delim_pos = header[1].1[iterator].0;
                    let last_delim_pos = {
                        if iterator == 0 {
                            0
                        } else {
                            header[1].1[iterator-1].0+args.delim.len()
                        }
                    };

                    value.push_str(&header[1].0[last_delim_pos..first_delim_pos]);
                    value.push_str(&word);
                    iterator += 2;
                }
                let last_delim_pos = header[1].1[iterator-1].0+args.delim.len();
                if last_delim_pos < header[1].0.len() {
                    value.push_str(&header[1].0[last_delim_pos..])
                }

            } else {
                value.push_str(&header[1].0);
            }
            req = req.header(&key, &value);
        }

        if let Some(body) = &bodies {
            let mut value: String = String::from("");
            let mut iterator = 0;
            if body.1.len() > 0 {
                while iterator < body.1.len() {
                    let first_delim_pos = body.1[iterator].0;
                    let last_delim_pos = {
                        if iterator == 0 {
                            0
                        } else {
                            body.1[iterator-1].0+args.delim.len()
                        }
                    };

                    value.push_str(&body.0[last_delim_pos..first_delim_pos]);
                    value.push_str(&word);
                    iterator += 2;
                }
                let last_delim_pos = body.1[iterator-1].0+args.delim.len();
                if last_delim_pos < body.0.len() {
                    value.push_str(&body.0[last_delim_pos..])
                }

            } else {
                value.push_str(&body.0);
            }
            println!("{}", value);
            req = req.body(value);
        }

        let resp = req.send().await?;

        let status = resp.status();
        let resp_headers = resp.headers().clone();
        let text = resp.text().await?;

        println!("Status code: {}. Length: {}. Word: {}", status, text.len(), word);
        if args.verbose {
            println!("{:#?}\n{:#}", resp_headers, text);
        }
    }

    Ok(())
}

async fn load_words_to_memory(filename: String, wordlist: Arc<Mutex<Vec<String>>>) {
    let line_iterator = io::BufReader::new(File::open(filename).unwrap()).lines();

    for line in line_iterator {
        let mut vec = wordlist.lock().unwrap();
        vec.push(line.unwrap());
    }
}