use clap::Parser;
use reqwest;
use std::sync::{Arc, Mutex};
use std::fs::File;
use std::io::{self, BufRead};
use std::thread;
use std::time;

/// Make an HTTP request repeatedly with a wordlist and receive data characteristics
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
    /// Example: -b '{"username":"john","password":"##123456##"}' -D "##"
    #[arg(short = 'D', long)]
    delim: String,

    /// Displays the response headers and body.
    /// 
    /// Example: -u "http://example.com" --verbose
    #[arg(short, long)]
    verbose: bool,

    /// Follows the redirect status codes.
    /// 
    /// Example: -u "http://example.com" --allowredirects
    #[arg(short, long)]
    allowredirects: bool,

    /// Amount of threads to use for sending http requests.
    /// 
    /// Example: -u "http://example.com" -t 4
    #[arg(short, long, default_value_t = 1)]
    threads: u16,

    /// Amount of delay in milliseconds between each request. Multiplied by amount of threads.
    /// 
    /// Example: -u "http://example.com" -d 100
    #[arg(short, long)]
    delay: Option<u16>,
}

fn main() {
    let args = Args::parse();

    // List of allowed methods to verify user input.
    let http_methods = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS", "CONNECT", "TRACE"];
    // This vec will contain all of the words from the wordlist.
    let wordlist = Arc::new(Mutex::new(Vec::<String>::new()));

    load_words_to_memory(args.list, Arc::clone(&wordlist));

    if !http_methods.contains(&args.method.as_str()) {
        panic!("Method not valid")
    }

    let mut handles = Vec::new();
    let now = time::Instant::now();
    for _ in 0..args.threads {
        let wordlist = Arc::clone(&wordlist);
        let handle = thread::spawn(move || {
            // Arguments are parsed again due to the move keyword.
            // This allows us to reuse the arguments every time a new thread is created without the
            // problem of ownership.
            let args = Args::parse();

            let mut headers: Vec<Vec<(String, Vec<(usize, _)>)>> = Vec::new();

            // This loop parses all headers, splits them into key and value, and detects
            // delimiters without replacing them.
            // Headers need to be split into key and value to pass them to the reqwest library.
            // This also verifies that the headers are valid by splitting them into 2 parts.
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
        
            // Parses the body, detecting the delimiters, same as the headers loop.
            let mut bodies: Option<(String, Vec<(usize, _)>)> = None;
            if let Some(body) = &args.body {
                let indices: Vec<(usize, _)> = body.match_indices(args.delim.as_str()).collect();
        
                if indices.len() % 2 != 0 {
                    panic!("Delimiters need to be set in pairs");
                }
        
                bodies = Some((body.clone(), indices));
            }

            // We need to create a client to disallow redirects. By default, reqwest follows all
            // redirects. This is detrimental depending on the performed activity, but by creating
            // a client there is extra overhead in performance.
            // We use blocking because of multithreading. By default, the library uses async tasks.
            let mut client = reqwest::blocking::ClientBuilder::new();
            if !args.allowredirects {
                client = client.redirect(reqwest::redirect::Policy::none());
            }
            let clientready = client.build().unwrap();

            loop {
                // This segment of code gets the vec of words, takes a word, and unlocks the vec.
                // This allows for the vec to be freed for other threads to use it immediately.
                let mut wordsmutex = wordlist.lock().unwrap();
                let word = match wordsmutex.pop() {
                    Some(word) => word,
                    None => break,
                };
                drop(wordsmutex);

                let mut req = clientready.request(
                    reqwest::Method::from_bytes(args.method.as_bytes()).unwrap(),
                     args.url.as_str()
                );

                // This loop is in charge of replacing the delimiters with the word from the
                // wordlist. We use the vec of already detected delimiters to facilitate it.
                for header in &headers {
                    let mut iterator: usize = 0;
                    let mut key: String = String::from("");
                    let mut value: String = String::from("");
                    // Checks if any delimiters were detected in the header key. If not, pushes the
                    // header as is.
                    if header[0].1.len() > 0 {
                        // Iterates each delimiter detected in pairs (2 delimiters need to surround
                        // the place where the word will go from the wordlist).
                        while iterator < header[0].1.len() {
                            // first_delim_pos = left delimiter which corresponds to the beginning
                            // of the word.
                            // last_delim_pos = last delimiter before the left most delimiter.
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

                        // Once we reach the end, we push to the string anything that is after the
                        // last delimiter inside the string (in this case the header key).
                        let last_delim_pos = header[0].1[iterator-1].0+args.delim.len();
                        if last_delim_pos < header[0].0.len() {
                            key.push_str(&header[0].0[last_delim_pos..])
                        }

                    } else {
                        key.push_str(&header[0].0);
                    }
                    iterator = 0;
                    // Works identically to the last iteration, but this time, checks the header
                    // value instead.
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
                    // Once everything is verified, pushes the key and value of the header.
                    req = req.header(&key, &value);
                }

                // Works similarly to the header verification of delimiters, without the need to
                // verify keys or values, as body only has a single string instance.
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
                    req = req.body(value);
                }
                // Sends the response, blocking the thread until receiving a reply.
                let resp = req.send().unwrap();

                let status = resp.status();
                let resp_headers = resp.headers().clone();
                let text = resp.text().unwrap();

                println!("Status code: {}. Length: {}. Word: {}", status, text.len(), word);
                if args.verbose {
                    println!("{:#?}\n{:#}", resp_headers, text);
                }
                // Delays the thread by amount equal to in threads. Multiplies the delay by the
                // amount of threads to account for batch requests that threads make
                // simultaneously.
                if let Some(delay) = &args.delay {
                    thread::sleep(time::Duration::from_millis((delay * args.threads) as u64));
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
    let elapsed = now.elapsed();
    println!("Complete! Time taken: {:.2?}", elapsed);
}

// Reads all words from a file and pushes them to the Vec in Arc Mutex. Allows for easier access
// later in the program.
fn load_words_to_memory(filename: String, wordlist: Arc<Mutex<Vec<String>>>) {
    let mut vec = wordlist.lock().unwrap();
    let line_iterator = io::BufReader::new(
        File::open(filename).unwrap()
    ).lines();

    for line in line_iterator {
        vec.push(line.unwrap());
    }
}