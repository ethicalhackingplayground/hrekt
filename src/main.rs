use async_std::io;
use async_std::io::prelude::*;
use clap::{App, Arg};
use colored::Colorize;
use futures::{stream::FuturesUnordered, StreamExt};
use governor::{Quota, RateLimiter};
use headless_chrome::Browser;
use regex;
use regex::Regex;
use reqwest::redirect;
use std::{error::Error, process::exit, time::Duration};
use tokio::{net, runtime::Builder, task};
use wappalyzer::{self};

#[derive(Clone, Debug)]
pub struct Job {
    host: Option<String>,
    body_regex: Option<String>,
    header_regex: Option<String>,
    ports: Option<String>,
    display_title: Option<bool>,
    display_tech: Option<bool>,
    path: Option<String>,
}

/**
 * Print the ascii banner
 */
fn print_banner() {
    const BANNER: &str = r#"   
  __  __     ______     ______     __  __     ______  
 /\ \_\ \   /\  == \   /\  ___\   /\ \/ /    /\__  _\ 
 \ \  __ \  \ \  __<   \ \  __\   \ \  _"-.  \/_/\ \/ 
  \ \_\ \_\  \ \_\ \_\  \ \_____\  \ \_\ \_\    \ \_\ 
   \/_/\/_/   \/_/ /_/   \/_____/   \/_/\/_/     \/_/ 
                                                                                                                     
                    v0.1.4                  
    "#;
    eprintln!("{}", BANNER.bold().cyan());
    eprintln!(
        "{}{}{} {}",
        "[".bold().white(),
        "WRN".bold().yellow(),
        "]".bold().white(),
        "Use with caution. You are responsible for your actions"
            .bold()
            .white()
    );
    eprintln!(
        "{}{}{} {}",
        "[".bold().white(),
        "WRN".bold().yellow(),
        "]".bold().white(),
        "Developers assume no liability and are not responsible for any misuse or damage."
            .bold()
            .white()
    );
    eprintln!(
        "{}{}{} {}\n",
        "[".bold().white(),
        "WRN".bold().yellow(),
        "]".bold().white(),
        "By using hrekt, you also agree to the terms of the APIs used."
            .bold()
            .white()
    );
}

/**
 * The main entry point
 */
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
    // parse the cli arguments
    let matches = App::new("hrekt")
        .version("0.1.4")
        .author("Blake Jacobs <krypt0mux@gmail.com>")
        .about("really fast http prober")
        .arg(
            Arg::with_name("rate")
                .short('r')
                .long("rate")
                .takes_value(true)
                .default_value("1000")
                .display_order(1)
                .help("Maximum in-flight requests per second"),
        )
        .arg(
            Arg::with_name("concurrency")
                .short('c')
                .long("concurrency")
                .default_value("100")
                .takes_value(true)
                .display_order(2)
                .help("The amount of concurrent requests"),
        )
        .arg(
            Arg::with_name("timeout")
                .short('t')
                .long("timeout")
                .default_value("3")
                .takes_value(true)
                .display_order(3)
                .help("The delay between each request"),
        )
        .arg(
            Arg::with_name("workers")
                .short('w')
                .long("workers")
                .default_value("1")
                .takes_value(true)
                .display_order(4)
                .help("The amount of workers"),
        )
        .arg(
            Arg::with_name("ports")
                .short('p')
                .long("ports")
                .default_value("80,443")
                .takes_value(true)
                .display_order(5)
                .help("the ports to probe default ports are (80,443)"),
        )
        .arg(
            Arg::with_name("title")
                .long("title")
                .short('i')
                .takes_value(false)
                .display_order(6)
                .help("display the page titles"),
        )
        .arg(
            Arg::with_name("tech-detect")
                .long("tech-detect")
                .short('d')
                .takes_value(false)
                .display_order(7)
                .help("display the technology used"),
        )
        .arg(
            Arg::with_name("path")
                .long("path")
                .short('x')
                .default_value("")
                .takes_value(true)
                .display_order(8)
                .help("probe the specified path"),
        )
        .arg(
            Arg::with_name("body-regex")
                .long("body-regex")
                .short('b')
                .default_value("")
                .takes_value(true)
                .display_order(9)
                .help("regex to be used to match a specific pattern in the response"),
        )
        .arg(
            Arg::with_name("header-regex")
                .long("header-regex")
                .short('h')
                .default_value("")
                .takes_value(true)
                .display_order(10)
                .help("regex to be used to match a specific pattern in the header"),
        )
        .arg(
            Arg::with_name("follow-redirects")
                .short('l')
                .long("follow-redirects")
                .takes_value(false)
                .display_order(11)
                .help("follow http redirects"),
        )
        .arg(
            Arg::with_name("silent")
                .short('q')
                .long("silent")
                .takes_value(false)
                .display_order(12)
                .help("suppress output"),
        )
        .get_matches();

    let silent = matches.is_present("silent");
    if !silent {
        print_banner();
    }

    let rate = match matches.value_of("rate").unwrap().parse::<u32>() {
        Ok(n) => n,
        Err(_) => {
            println!("{}", "could not parse rate, using default of 1000");
            1000
        }
    };

    let body_regex = match matches.value_of("body-regex").unwrap().parse::<String>() {
        Ok(body_regex) => body_regex,
        Err(_) => "".to_string(),
    };

    let header_regex = match matches.value_of("header-regex").unwrap().parse::<String>() {
        Ok(header_regex) => header_regex,
        Err(_) => "".to_string(),
    };

    let ports = match matches.value_of("ports").unwrap().parse::<String>() {
        Ok(ports) => ports,
        Err(_) => "".to_string(),
    };

    let path = match matches.value_of("path").unwrap().parse::<String>() {
        Ok(path) => path,
        Err(_) => "".to_string(),
    };

    let display_title = matches.is_present("title");
    let display_tech = matches.is_present("tech-detect");
    let follow_redirects = matches.is_present("follow-redirects");

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
    rt.spawn(async move {
        send_url(
            job_tx,
            body_regex,
            header_regex,
            ports,
            display_title,
            display_tech,
            path,
            rate,
        )
        .await
    });

    // process the jobs
    let workers = FuturesUnordered::new();

    // process the jobs for scanning.
    for _ in 0..concurrency {
        let jrx = job_rx.clone();
        // initialize the new chromium browser instance
        let port = match port_selector::random_free_tcp_port() {
            Some(port) => port,
            None => {
                println!("Something bad happened :(");
                exit(1);
            }
        };
        let browser = wappalyzer::new_browser(port);
        let browser_instance = browser.clone();
        workers.push(task::spawn(async move {
            //  run the detector
            run_detector(jrx, follow_redirects, browser_instance, timeout).await
        }));
    }
    let _: Vec<_> = workers.collect().await;
    rt.shutdown_background();

    Ok(())
}

/**
 * Send the urls to be processed by the workers
 */
async fn send_url(
    mut tx: spmc::Sender<Job>,
    body_regex: String,
    header_regex: String,
    ports: String,
    display_title: bool,
    display_tech: bool,
    path: String,
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
            body_regex: Some(body_regex.clone()),
            header_regex: Some(header_regex.clone()),
            ports: Some(ports.to_string()),
            display_title: Some(display_title.clone()),
            display_tech: Some(display_tech.clone()),
            path: Some(path.clone()),
        };
        if let Err(err) = tx.send(msg) {
            eprintln!("{}", err.to_string());
        }
        // send the jobs
        lim.until_ready().await;
    }
    Ok(())
}

/**
 * Perform the HTTP probing operation.
 */
pub async fn run_detector(
    rx: spmc::Receiver<Job>,
    follow_redirects: bool,
    browser: Browser,
    timeout: usize,
) {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_static(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:95.0) Gecko/20100101 Firefox/95.0",
        ),
    );

    let client;
    if follow_redirects {
        //no certs
        client = reqwest::Client::builder()
            .default_headers(headers)
            .redirect(redirect::Policy::limited(10))
            .timeout(Duration::from_secs(timeout.try_into().unwrap()))
            .danger_accept_invalid_hostnames(true)
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap();
    } else {
        //no certs
        client = reqwest::Client::builder()
            .default_headers(headers)
            .redirect(redirect::Policy::none())
            .timeout(Duration::from_secs(timeout.try_into().unwrap()))
            .danger_accept_invalid_hostnames(true)
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap();
    }

    while let Ok(job) = rx.recv() {
        let job_host: String = job.host.unwrap();
        let job_body_regex = job.body_regex.unwrap();
        let job_header_regex = job.header_regex.unwrap();
        let job_path = job.path.unwrap();
        let job_ports = job.ports.unwrap();
        let job_title = job.display_title.unwrap();
        let job_tech = job.display_tech.unwrap();
        let mut resolved_domains: Vec<String> = vec![String::from("")];

        // probe for open ports and perform dns resolution
        let ports_array = job_ports.split(",");
        for (_, port) in ports_array.enumerate() {
            let job_host_http = job_host.clone();
            let job_host_https = job_host_http.clone();
            let http_port = port.to_string();
            let https_port = http_port.to_string();
            if port == "80" {
                let http = http_resolver(job_host_http, "http://".to_owned(), http_port).await;
                resolved_domains.push(http);
            } else if port == "443" {
                let https = http_resolver(job_host_https, "https://".to_owned(), https_port).await;
                resolved_domains.push(https);
            } else {
                let https =
                    http_resolver(job_host_https, "https://".to_owned(), https_port.to_owned())
                        .await;
                resolved_domains.push(https);

                let http = http_resolver(job_host_http, "http://".to_owned(), http_port).await;
                resolved_domains.push(http);
            }
        }

        // Iterate over the resolved IP addresses and send HTTP requests
        for domain in &resolved_domains {
            let domain_cp = domain.clone();
            if job_path != "" {
                let path_url = String::from(format!("{}{}", domain, job_path));
                let url = path_url.clone();
                let mut domain_result_url = String::from("");

                let path_resp_get = client.get(path_url);
                let path_resp_req = match path_resp_get.build() {
                    Ok(path_resp_req) => path_resp_req,
                    Err(_) => {
                        continue;
                    }
                };
                let path_resp = match client.execute(path_resp_req).await {
                    Ok(path_resp) => path_resp,
                    Err(_) => {
                        continue;
                    }
                };

                // check if a valid path has been found
                if path_resp.status().as_u16() != 404 && path_resp.status().as_u16() != 400 {
                    let browser_instance = browser.clone();
                    domain_result_url.push_str(&url);
                    let domain_result = domain_result_url.clone();
                    let get = client.get(domain_result_url);
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

                    // perform the regex on the headers
                    let mut header_match = String::from("");
                    let headers = resp.headers();
                    for (k, v) in headers.iter() {
                        let header_value = match v.to_str() {
                            Ok(header_value) => header_value,
                            Err(_) => "",
                        };
                        let header_str =
                            String::from(format!("{}:{}", k.as_str().to_string(), header_value));
                        let re = match regex::Regex::new(&job_header_regex) {
                            Ok(re) => re,
                            Err(_) => continue,
                        };
                        for m_str in re.captures_iter(&header_str) {
                            if m_str.len() > 0 {
                                let str_match = m_str[m_str.len() - 1].to_string();
                                header_match.push_str(&str_match);
                            }
                        }
                    }

                    let body = match resp.text().await {
                        Ok(body) => body,
                        Err(_) => {
                            continue;
                        }
                    };

                    // extract the page title
                    let mut title = String::from("");
                    if job_title {
                        let re = match Regex::new("<title>(.*)</title>") {
                            Ok(re) => re,
                            Err(_) => continue,
                        };
                        for cap in re.captures_iter(&body) {
                            if cap.len() > 0 {
                                title.push_str(&cap[1].to_string());
                                break;
                            }
                        }
                    }

                    // perform the regex on the response body
                    let re = match regex::Regex::new(&job_body_regex) {
                        Ok(re) => re,
                        Err(_) => continue,
                    };

                    let url = match reqwest::Url::parse(&domain_result) {
                        Ok(url) => url,
                        Err(_) => continue,
                    };

                    // extract the technologies
                    let mut tech_str = String::from("");
                    if job_tech {
                        let tech_analysis = wappalyzer::scan(url, &browser_instance).await;
                        let tech_result = match tech_analysis.result {
                            Ok(tech_result) => tech_result,
                            Err(_) => continue,
                        };
                        for tech in tech_result.iter() {
                            tech_str.push_str(&tech.name);
                            tech_str.push_str(",");
                        }
                    }
                    let tech = match tech_str.strip_suffix(",") {
                        Some(tech) => tech.to_string(),
                        None => "".to_string(),
                    };

                    let mut body_match = String::from("");
                    for m_str in re.captures_iter(&body) {
                        if m_str.len() > 0 {
                            let str_match = m_str[m_str.len() - 1].to_string();
                            body_match.push_str(&str_match);
                            break;
                        }
                    }
                    // print the final results
                    println!(
                        "{} {} [{}] {} {} [{}] {} {} [{}] {} {} [{}]",
                        domain_result.white().bold(),
                        "Title:".bold().white(),
                        title.white().bold(),
                        "::".bold().white(),
                        "Resp:".bold().white(),
                        body_match.white().bold(),
                        "::".bold().white(),
                        "Header:".bold().white(),
                        header_match.white().bold(),
                        "::".bold().white(),
                        "Tech:".bold().white(),
                        tech.white().bold()
                    );
                }
            } else {
                let browser_instance = browser.clone();
                let url = String::from(domain_cp);
                let domain_result = url.clone();
                let get = client.get(url);
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

                let mut header_match = String::from("");
                let headers = resp.headers();
                for (k, v) in headers.iter() {
                    let header_value = match v.to_str() {
                        Ok(header_value) => header_value,
                        Err(_) => "",
                    };
                    let header_str =
                        String::from(format!("{}:{}", k.as_str().to_string(), header_value));
                    let re = match regex::Regex::new(&job_header_regex) {
                        Ok(re) => re,
                        Err(_) => continue,
                    };
                    for m_str in re.captures_iter(&header_str) {
                        if m_str.len() > 0 {
                            let str_match = m_str[m_str.len() - 1].to_string();
                            header_match.push_str(&str_match);
                        }
                    }
                }

                let body = match resp.text().await {
                    Ok(body) => body,
                    Err(_) => {
                        continue;
                    }
                };

                let mut title = String::from("");
                if job_title {
                    let re = match Regex::new("<title>(.*)</title>") {
                        Ok(re) => re,
                        Err(_) => continue,
                    };
                    for cap in re.captures_iter(&body) {
                        if cap.len() > 0 {
                            title.push_str(&cap[1].to_string());
                            break;
                        }
                    }
                }

                let re = match regex::Regex::new(&job_body_regex) {
                    Ok(re) => re,
                    Err(_) => continue,
                };

                let url = match reqwest::Url::parse(&domain_result) {
                    Ok(url) => url,
                    Err(_) => continue,
                };

                let mut tech_str = String::from("");
                if job_tech {
                    let tech_analysis = wappalyzer::scan(url, &browser_instance).await;
                    let tech_result = match tech_analysis.result {
                        Ok(tech_result) => tech_result,
                        Err(_) => continue,
                    };
                    for tech in tech_result.iter() {
                        tech_str.push_str(&tech.name);
                        tech_str.push_str(",");
                    }
                }
                let tech = match tech_str.strip_suffix(",") {
                    Some(tech) => tech.to_string(),
                    None => "".to_string(),
                };

                let mut body_match = String::from("");
                for m_str in re.captures_iter(&body) {
                    if m_str.len() > 0 {
                        let str_match = m_str[m_str.len() - 1].to_string();
                        body_match.push_str(&str_match);
                        break;
                    }
                }
                // print the final results
                println!(
                    "{} {} [{}] {} {} [{}] {} {} [{}] {} {} [{}]",
                    domain_result.white().bold(),
                    "Title:".bold().white(),
                    title.white().bold(),
                    "::".bold().white(),
                    "Resp:".bold().white(),
                    body_match.white().bold(),
                    "::".bold().white(),
                    "Header:".bold().white(),
                    header_match.white().bold(),
                    "::".bold().white(),
                    "Tech:".bold().white(),
                    tech.white().bold()
                );
            }
        }
    }
}

/**
 * Resolve the subdomains and return the host
 */
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
