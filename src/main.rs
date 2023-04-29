use async_std::io;
use async_std::io::prelude::*;
use std::{error::Error, time::Duration};

use regex;

use clap::{App, Arg};
use futures::{stream::FuturesUnordered, StreamExt};
use governor::{Quota, RateLimiter};
use regex::Regex;
use reqwest::redirect;
use tokio::{net, runtime::Builder, task};

#[derive(Clone, Debug)]
pub struct Job {
    host: Option<String>,
    regex: Option<String>,
    ports: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
    // parse the cli arguments
    let matches = App::new("hrekt")
        .version("0.1.3")
        .author("Blake Jacobs <krypt0mux@gmail.com>")
        .about("really fast http prober")
        .arg(
            Arg::with_name("rate")
                .short('c')
                .long("rate")
                .takes_value(true)
                .default_value("1000")
                .display_order(2)
                .help("Maximum in-flight requests per second"),
        )
        .arg(
            Arg::with_name("concurrency")
                .short('t')
                .long("concurrency")
                .default_value("100")
                .takes_value(true)
                .display_order(3)
                .help("The amount of concurrent requests"),
        )
        .arg(
            Arg::with_name("regex")
                .short('r')
                .long("regex")
                .default_value("")
                .takes_value(true)
                .display_order(6)
                .help("regex to be used to match a specific pattern in the response"),
        )
        .arg(
            Arg::with_name("ports")
                .short('p')
                .long("ports")
                .default_value("80,443")
                .takes_value(true)
                .display_order(6)
                .help("the ports to probe default is (80,443)"),
        )
        .arg(
            Arg::with_name("timeout")
                .short('t')
                .long("timeout")
                .default_value("3")
                .takes_value(true)
                .display_order(4)
                .help("The delay between each request"),
        )
        .arg(
            Arg::with_name("workers")
                .short('w')
                .long("workers")
                .default_value("1")
                .takes_value(true)
                .display_order(5)
                .help("The amount of workers"),
        )
        .get_matches();

    let rate = match matches.value_of("rate").unwrap().parse::<u32>() {
        Ok(n) => n,
        Err(_) => {
            println!("{}", "could not parse rate, using default of 1000");
            1000
        }
    };

    let regex = match matches.value_of("regex").unwrap().parse::<String>() {
        Ok(regex) => regex,
        Err(_) => "".to_string(),
    };

    let ports = match matches.value_of("ports").unwrap().parse::<String>() {
        Ok(ports) => ports,
        Err(_) => "".to_string(),
    };

    let concurrency = match matches.value_of("concurrency").unwrap().parse::<u32>() {
        Ok(n) => n,
        Err(_) => {
            println!("{}", "could not parse concurrency, using default of 100");
            100
        }
    };

    let timeout = match matches.get_one::<String>("timeout").map(|s| s.to_string()) {
        Some(timeout) => timeout.parse::<usize>().unwrap(),
        None => 3,
    };

    let w: usize = match matches.value_of("workers").unwrap().parse::<usize>() {
        Ok(w) => w,
        Err(_) => {
            println!("{}", "could not parse workers, using default of 1");
            1
        }
    };

    // Set up a worker pool with the number of threads specified from the arguments
    let rt = Builder::new_multi_thread()
        .enable_all()
        .worker_threads(w)
        .build()
        .unwrap();

    // job channels
    let (job_tx, job_rx) = spmc::channel::<Job>();
    rt.spawn(async move { send_url(job_tx, regex, ports, rate).await });

    // process the jobs
    let workers = FuturesUnordered::new();

    // process the jobs for scanning.
    for _ in 0..concurrency {
        let jrx = job_rx.clone();
        workers.push(task::spawn(async move {
            //  run the detector
            run_detector(jrx, timeout).await
        }));
    }
    let _: Vec<_> = workers.collect().await;
    rt.shutdown_background();

    Ok(())
}

async fn send_url(
    mut tx: spmc::Sender<Job>,
    regex: String,
    ports: String,
    rate: u32,
) -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
    //set rate limit
    let lim = RateLimiter::direct(Quota::per_second(std::num::NonZeroU32::new(rate).unwrap()));

    let stdin = io::BufReader::new(io::stdin());
    let mut lines = stdin.lines();
    while let Some(line) = lines.next().await {
        let last_input = line.unwrap();
        let msg = Job {
            host: Some(last_input.to_string().clone()),
            regex: Some(regex.clone()),
            ports: Some(ports.to_string()),
        };
        if let Err(err) = tx.send(msg) {
            eprintln!("{}", err.to_string());
        }
        // send the jobs
        lim.until_ready().await;
    }
    Ok(())
}

// this function will test perform the aem detection
pub async fn run_detector(rx: spmc::Receiver<Job>, timeout: usize) {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_static(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:95.0) Gecko/20100101 Firefox/95.0",
        ),
    );

    //no certs
    let client = reqwest::Client::builder()
        .default_headers(headers)
        .redirect(redirect::Policy::limited(10))
        .timeout(Duration::from_secs(timeout.try_into().unwrap()))
        .danger_accept_invalid_hostnames(true)
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();

    while let Ok(job) = rx.recv() {
        let job_host: String = job.host.unwrap();
        let job_regex = job.regex.unwrap();
        let job_ports = job.ports.unwrap();
        let mut resolved_domains: Vec<String> = vec![String::from("")];

        let ports_array = job_ports.split(",");
        for (_, port) in ports_array.enumerate() {
            let job_host_http = job_host.clone();
            let job_host_https = job_host_http.clone();
            let http_port = port.to_string();
            let https_port = http_port.to_string();
            let http = http_resolver(job_host_http, "http://".to_owned(), http_port).await;
            resolved_domains.push(http);

            let https = http_resolver(job_host_https, "https://".to_owned(), https_port).await;
            resolved_domains.push(https);
        }
        for domain in &resolved_domains {
            let domain_result = domain.clone();
            // Iterate over the resolved IP addresses and send HTTP requests
            let get = client.get(domain);
            let req = match get.build() {
                Ok(req) => req,
                Err(_) => {
                    continue;
                }
            };
            let resp = match client.execute(req).await {
                Ok(resp) => resp,
                Err(_) => {
                    continue;
                }
            };
            let body = match resp.text().await {
                Ok(body) => body,
                Err(_) => {
                    continue;
                }
            };
            let mut title = String::from("");
            let re = Regex::new("<title>(.*)</title>").unwrap();
            for cap in re.captures_iter(&body) {
                if cap.len() > 0 {
                    title.push_str(&cap[1].to_string());
                    break;
                }
            }

            let re = match regex::Regex::new(&job_regex) {
                Ok(re) => re,
                Err(_) => continue,
            };

            for m_str in re.captures_iter(&body) {
                if m_str.len() > 0 {
                    let str_match = m_str[m_str.len() - 1].to_string();
                    println!("{} [{}] [{}]", domain_result, title, str_match);
                    break;
                } else {
                    println!("{} [{}]", domain_result, title);
                    break;
                }
            }
        }
    }
}

async fn http_resolver(host: String, schema: String, port: String) -> String {
    let mut host_str = String::from(schema);
    let domain = String::from(format!("{}:{}", host, port));
    let lookup = match net::lookup_host(domain).await {
        Ok(lookup) => lookup,
        Err(_) => return "".to_string(),
    };
    // Perform DNS resolution to get IP addresses for the hostname

    for addr in lookup {
        if addr.is_ipv4() {
            host_str.push_str(&host);
            host_str.push_str(":");
            host_str.push_str(&port.to_string());
            break;
        }
    }
    return host_str;
}
