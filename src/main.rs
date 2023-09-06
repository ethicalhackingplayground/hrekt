use async_std::io;
use async_std::io::prelude::*;
use clap::{Arg, ArgAction, Command};
use colored::Colorize;
use futures::{stream::FuturesUnordered, StreamExt};
use governor::{Quota, RateLimiter};
use headless_chrome::Browser;
use regex;
use regex::Regex;
use reqwest::redirect;
use std::{error::Error, time::Duration};
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
    status_codes: Option<bool>,
    content_length: Option<bool>,
    content_type: Option<bool>,
    server: Option<bool>,
    path: Option<String>,
}

/**
 * Print the ascii banner
 */
fn print_banner() {
    const BANNER: &str = r#"   
    __              __   __ 
   / /_  ________  / /__/ /_
  / __ \/ ___/ _ \/ //_/ __/
 / / / / /  /  __/ ,< / /_  
/_/ /_/_/   \___/_/|_|\__/  
                            
                v0.1.6
    "#;
    eprintln!("{}", BANNER.white());
}

/**
 * The main entry point
 */
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
    // parse the cli arguments
    let matches = Command::new("hrekt")
        .version("0.1.6")
        .author("Blake Jacobs <krypt0mux@gmail.com>")
        .about("really fast http prober")
        .arg(
            Arg::new("rate")
                .short('r')
                .long("rate")
                .default_value("1000")
                .display_order(1)
                .help("Maximum in-flight requests per second"),
        )
        .arg(
            Arg::new("concurrency")
                .short('c')
                .long("concurrency")
                .default_value("100")
                .display_order(2)
                .help("The amount of concurrent requests"),
        )
        .arg(
            Arg::new("timeout")
                .short('t')
                .long("timeout")
                .default_value("3")
                .display_order(3)
                .help("The delay between each request"),
        )
        .arg(
            Arg::new("workers")
                .short('w')
                .long("workers")
                .default_value("1")
                .display_order(4)
                .help("The amount of workers"),
        )
        .arg(
            Arg::new("ports")
                .short('p')
                .long("ports")
                .default_value("80,443")
                .display_order(5)
                .help("the ports to probe default ports are (80,443)"),
        )
        .arg(
            Arg::new("title")
                .long("title")
                .hide_short_help(true)
                .action(ArgAction::SetTrue)
                .display_order(6)
                .help("display the page titles"),
        )
        .arg(
            Arg::new("tech-detect")
                .long("tech-detect")
                .hide_short_help(true)
                .action(ArgAction::SetTrue)
                .display_order(7)
                .help("display the technology used"),
        )
        .arg(
            Arg::new("status-code")
                .long("status-code")
                .hide_short_help(true)
                .action(ArgAction::SetTrue)
                .display_order(8)
                .help("display the status-codes"),
        )
        .arg(
            Arg::new("server")
                .long("server")
                .action(ArgAction::SetTrue)
                .display_order(9)
                .help("displays the server"),
        )
        .arg(
            Arg::new("content-type")
                .long("content-type")
                .hide_short_help(true)
                .action(ArgAction::SetTrue)
                .display_order(10)
                .help("displays the content type"),
        )
        .arg(
            Arg::new("content-length")
                .long("content-length")
                .hide_short_help(true)
                .action(ArgAction::SetTrue)
                .display_order(11)
                .help("displays the content length"),
        )
        .arg(
            Arg::new("path")
                .long("path")
                .short('x')
                .default_value("")
                .display_order(12)
                .help("probe the specified path"),
        )
        .arg(
            Arg::new("body-regex")
                .long("body-regex")
                .hide_short_help(true)
                .default_value("")
                .display_order(13)
                .help("regex to be used to match a specific pattern in the response"),
        )
        .arg(
            Arg::new("header-regex")
                .long("header-regex")
                .hide_short_help(true)
                .default_value("")
                .display_order(14)
                .help("regex to be used to match a specific pattern in the header"),
        )
        .arg(
            Arg::new("follow-redirects")
                .short('l')
                .long("follow-redirects")
                .action(ArgAction::SetTrue)
                .display_order(15)
                .help("follow http redirects"),
        )
        .arg(
            Arg::new("silent")
                .short('q')
                .long("silent")
                .action(ArgAction::SetTrue)
                .display_order(16)
                .help("suppress output"),
        )
        .get_matches();

    let silent = matches.get_flag("silent");
    if !silent {
        print_banner();
    }

    let status_codes = matches.get_flag("status-code");

    let rate = match matches.get_one::<String>("rate").unwrap().parse::<String>() {
        Ok(n) => n.parse::<u32>().unwrap(),
        Err(_) => {
            println!("{}", "could not parse rate, using default of 1000");
            1000
        }
    };

    let body_regex = match matches
        .get_one::<String>("body-regex")
        .unwrap()
        .parse::<String>()
    {
        Ok(body_regex) => body_regex,
        Err(_) => "".to_string(),
    };

    let header_regex = match matches
        .get_one::<String>("header-regex")
        .unwrap()
        .parse::<String>()
    {
        Ok(header_regex) => header_regex,
        Err(_) => "".to_string(),
    };

    let ports = match matches
        .get_one::<String>("ports")
        .unwrap()
        .parse::<String>()
    {
        Ok(ports) => ports,
        Err(_) => "".to_string(),
    };

    let path = match matches.get_one::<String>("path").unwrap().parse::<String>() {
        Ok(path) => path,
        Err(_) => "".to_string(),
    };

    let display_title = matches.get_flag("title");
    let display_tech = matches.get_flag("tech-detect");
    let follow_redirects = matches.get_flag("follow-redirects");
    let content_length = matches.get_flag("content-length");
    let content_type = matches.get_flag("content-type");
    let server = matches.get_flag("server");

    let concurrency = match matches
        .get_one::<String>("concurrency")
        .map(|s| s.to_string())
    {
        Some(n) => match n.parse::<i32>() {
            Ok(n) => n,
            Err(_) => 100,
        },
        None => {
            println!("{}", "could not parse concurrency, using default of 100");
            100
        }
    };

    let timeout = match matches.get_one::<String>("timeout").map(|s| s.to_string()) {
        Some(timeout) => match timeout.parse::<usize>() {
            Ok(timeout) => timeout,
            Err(_) => 3,
        },
        None => 3,
    };

    let w: usize = match matches.get_one::<String>("workers").map(|s| s.to_string()) {
        Some(w) => match w.parse::<usize>() {
            Ok(w) => w,
            Err(_) => 1,
        },
        None => {
            println!("{}", "could not parse workers, using default of 1");
            1
        }
    };

    // collect hosts from stdin
    let mut hosts = vec![];
    let stdin = io::BufReader::new(io::stdin());
    let mut lines = stdin.lines();
    while let Some(line) = lines.next().await {
        let host = match line {
            Ok(host) => host,
            Err(_) => "".to_string(),
        };
        hosts.push(host);
    }

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
            hosts,
            body_regex,
            header_regex,
            ports,
            display_title,
            display_tech,
            status_codes,
            content_type,
            content_length,
            server,
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
                continue;
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
    hosts: Vec<String>,
    body_regex: String,
    header_regex: String,
    ports: String,
    display_title: bool,
    display_tech: bool,
    status_codes: bool,
    content_type: bool,
    content_length: bool,
    server: bool,
    path: String,
    rate: u32,
) -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
    //set rate limit
    let lim = RateLimiter::direct(Quota::per_second(std::num::NonZeroU32::new(rate).unwrap()));

    for host in hosts.iter() {
        // send the jobs
        lim.until_ready().await;
        let msg = Job {
            host: Some(host.to_string().clone()),
            body_regex: Some(body_regex.clone()),
            header_regex: Some(header_regex.clone()),
            ports: Some(ports.to_string()),
            display_title: Some(display_title.clone()),
            display_tech: Some(display_tech.clone()),
            path: Some(path.clone()),
            status_codes: Some(status_codes.clone()),
            content_length: Some(content_length.clone()),
            content_type: Some(content_type.clone()),
            server: Some(server.clone()),
        };
        if let Err(err) = tx.send(msg) {
            eprintln!("{}", err.to_string());
        }
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
        let job_status_codes = job.status_codes.unwrap();
        let job_content_length = job.content_length.unwrap();
        let job_content_type = job.content_type.unwrap();
        let job_server = job.server.unwrap();
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
                    let domain_result_cloned = domain_result.clone();
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

                    let mut content_length = String::from("");

                    if job_content_length {
                        let domain_result_2 = domain_result_cloned.clone();
                        let get_request = client.get(domain_result_2);
                        let request = match get_request.build() {
                            Ok(req) => req,
                            Err(_) => {
                                continue;
                            }
                        };
                        let response = match client.execute(request).await {
                            Ok(resp) => resp,
                            Err(_) => {
                                continue;
                            }
                        };
                        content_length.push_str("[");
                        let cl = match response.content_length() {
                            Some(cl) => cl.to_string(),
                            None => "".to_string(),
                        };
                        content_length.push_str(&cl);
                        content_length.push_str("]");
                    }

                    let mut content_type = String::from("");

                    if job_content_type {
                        let domain_result_2 = domain_result_cloned.clone();
                        let get_request = client.get(domain_result_2);
                        let request = match get_request.build() {
                            Ok(req) => req,
                            Err(_) => {
                                continue;
                            }
                        };
                        let response = match client.execute(request).await {
                            Ok(resp) => resp,
                            Err(_) => {
                                continue;
                            }
                        };
                        let ct = match response.headers().get("Content-Type") {
                            Some(ct) => match ct.to_str() {
                                Ok(ct) => ct.to_string(),
                                Err(_) => continue,
                            },
                            None => "".to_string(),
                        };
                        if !ct.is_empty() {
                            content_type.push_str("[");
                            content_type.push_str(&ct);
                            content_type.push_str("]");
                        }
                    }

                    let mut server = String::from("");

                    if job_server {
                        let domain_result_2 = domain_result_cloned.clone();
                        let get_request = client.get(domain_result_2);
                        let request = match get_request.build() {
                            Ok(req) => req,
                            Err(_) => {
                                continue;
                            }
                        };
                        let response = match client.execute(request).await {
                            Ok(resp) => resp,
                            Err(_) => {
                                continue;
                            }
                        };
                        let s = match response.headers().get("Server") {
                            Some(s) => match s.to_str() {
                                Ok(s) => s.to_string(),
                                Err(_) => continue,
                            },
                            None => "".to_string(),
                        };
                        if !server.is_empty() {
                            server.push_str("[");
                            server.push_str(&s);
                            server.push_str("]");
                        }
                    }

                    let get_request = client.get(domain_result_cloned);
                    let request = match get_request.build() {
                        Ok(req) => req,
                        Err(_) => {
                            continue;
                        }
                    };
                    let response = match client.execute(request).await {
                        Ok(resp) => resp,
                        Err(_) => {
                            continue;
                        }
                    };

                    // perform the regex on the headers
                    if !job_header_regex.is_empty() {
                        let headers = resp.headers();
                        for (k, v) in headers.iter() {
                            let header_value = match v.to_str() {
                                Ok(header_value) => header_value,
                                Err(_) => "",
                            };
                            let header_str = String::from(format!(
                                "{}:{}",
                                k.as_str().to_string(),
                                header_value
                            ));
                            let re = match regex::Regex::new(&job_header_regex) {
                                Ok(re) => re,
                                Err(_) => continue,
                            };
                            if !re.is_match(&header_str) {
                                continue;
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
                                if !cap[1].to_string().is_empty() {
                                    title.push_str("[");
                                    title.push_str(&cap[1].to_string());
                                    title.push_str("]");
                                    break;
                                }
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
                        let mut tech_name = String::from("");
                        for tech in tech_result.iter() {
                            tech_name.push_str(&tech.name);
                            tech_name.push_str(",");
                        }
                        if !tech_name.is_empty() {
                            tech_str.push_str("[");
                            let tech = match tech_name.strip_suffix(",") {
                                Some(tech) => tech.to_string(),
                                None => "".to_string(),
                            };
                            tech_str.push_str(&tech.to_string());
                            tech_str.push_str("]");
                        }
                    }

                    if !job_body_regex.is_empty() {
                        if !re.is_match(&body) {
                            continue;
                        }
                    }

                    let mut status_code = String::from("");
                    if job_status_codes {
                        let sc = response.status().as_u16();
                        status_code.push_str("[");
                        status_code.push_str(&sc.to_string());
                        status_code.push_str("]");
                        if sc >= 100 && sc < 200 {
                            // print the final results
                            println!(
                                "{} {} {} {} {} {} {}",
                                domain_result,
                                title.cyan(),
                                status_code.white(),
                                tech_str.white().bold(),
                                content_type,
                                content_length,
                                server
                            );
                        }
                        if sc >= 200 && sc < 300 {
                            // print the final results
                            println!(
                                "{} {} {} {} {} {} {}",
                                domain_result,
                                title.cyan(),
                                status_code.green(),
                                tech_str.white().bold(),
                                content_type,
                                content_length,
                                server
                            );
                        }
                        if sc >= 300 && sc < 400 {
                            // print the final results
                            println!(
                                "{} {} {} {} {} {} {}",
                                domain_result,
                                title.cyan(),
                                status_code.blue(),
                                tech_str.white().bold(),
                                content_type,
                                content_length,
                                server
                            );
                        }
                        if sc >= 400 && sc < 500 {
                            // print the final results
                            println!(
                                "{} {} {} {} {} {} {}",
                                domain_result,
                                title.cyan(),
                                status_code.magenta(),
                                tech_str.white().bold(),
                                content_type,
                                content_length,
                                server
                            );
                        }
                        if sc >= 500 && sc < 600 {
                            // print the final results
                            println!(
                                "{} {} {} {} {} {} {}",
                                domain_result,
                                title.cyan(),
                                status_code.red(),
                                tech_str.white().bold(),
                                content_type,
                                content_length,
                                server
                            );
                        }
                    } else {
                        // print the final results
                        println!(
                            "{} {} {} {} {} {} {}",
                            domain_result,
                            title.cyan(),
                            status_code.red(),
                            tech_str.white().bold(),
                            content_type,
                            content_length,
                            server
                        );
                    }
                }
            } else {
                let browser_instance = browser.clone();
                let url = String::from(domain_cp);
                let url_cloned = url.clone();
                let domain_result = url.clone();
                let domain_result_cloned = domain_result.clone();
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

                let get_request = client.get(url_cloned);
                let request = match get_request.build() {
                    Ok(req) => req,
                    Err(_) => {
                        continue;
                    }
                };
                let response = match client.execute(request).await {
                    Ok(resp) => resp,
                    Err(_) => {
                        continue;
                    }
                };

                let mut content_length = String::from("");

                if job_content_length {
                    let domain_result_cloned_2 = domain_result_cloned.clone();
                    let get_request = client.get(domain_result_cloned_2);
                    let request = match get_request.build() {
                        Ok(req) => req,
                        Err(_) => {
                            continue;
                        }
                    };
                    let response = match client.execute(request).await {
                        Ok(resp) => resp,
                        Err(_) => {
                            continue;
                        }
                    };
                    content_length.push_str("[");
                    let cl = match response.content_length() {
                        Some(cl) => cl.to_string(),
                        None => "".to_string(),
                    };
                    content_length.push_str(&cl);
                    content_length.push_str("]");
                }

                let mut content_type = String::from("");

                if job_content_type {
                    let domain_result_cloned_2 = domain_result_cloned.clone();
                    let get_request = client.get(domain_result_cloned_2);
                    let request = match get_request.build() {
                        Ok(req) => req,
                        Err(_) => {
                            continue;
                        }
                    };
                    let response = match client.execute(request).await {
                        Ok(resp) => resp,
                        Err(_) => {
                            continue;
                        }
                    };

                    let ct = match response.headers().get("Content-Type") {
                        Some(ct) => match ct.to_str() {
                            Ok(ct) => ct.to_string(),
                            Err(_) => continue,
                        },
                        None => "".to_string(),
                    };
                    if !ct.is_empty() {
                        content_type.push_str("[");
                        content_type.push_str(&ct);
                        content_type.push_str("]");
                    }
                }
                let mut server = String::from("");
                if job_server {
                    let domain_result_cloned_2 = domain_result_cloned.clone();
                    let get_request = client.get(domain_result_cloned_2);
                    let request = match get_request.build() {
                        Ok(req) => req,
                        Err(_) => {
                            continue;
                        }
                    };
                    let response = match client.execute(request).await {
                        Ok(resp) => resp,
                        Err(_) => {
                            continue;
                        }
                    };
                    let s = match response.headers().get("Server") {
                        Some(s) => match s.to_str() {
                            Ok(s) => s.to_string(),
                            Err(_) => continue,
                        },
                        None => "".to_string(),
                    };
                    if !s.is_empty() {
                        server.push_str("[");
                        server.push_str(&s);
                        server.push_str("]");
                    }
                }

                if !job_header_regex.is_empty() {
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
                        if !re.is_match(&header_str) {
                            continue;
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
                            if !cap[1].to_string().is_empty() {
                                title.push_str("[");
                                title.push_str(&cap[1].to_string());
                                title.push_str("]");
                                break;
                            }
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
                    let mut tech_name = String::from("");
                    for tech in tech_result.iter() {
                        tech_name.push_str(&tech.name);
                        tech_name.push_str(",");
                    }
                    if !tech_name.is_empty() {
                        tech_str.push_str("[");
                        let tech = match tech_name.strip_suffix(",") {
                            Some(tech) => tech.to_string(),
                            None => "".to_string(),
                        };
                        tech_str.push_str(&tech.to_string());
                        tech_str.push_str("]");
                    }
                }

                if !job_body_regex.is_empty() {
                    if !re.is_match(&body) {
                        continue;
                    }
                }

                let mut status_code = String::from("");
                if job_status_codes {
                    let sc = response.status().as_u16();
                    status_code.push_str("[");
                    status_code.push_str(&sc.to_string());
                    status_code.push_str("]");
                    if sc >= 100 && sc < 200 {
                        // print the final results
                        println!(
                            "{} {} {} {} {} {} {}",
                            domain_result,
                            title.cyan(),
                            status_code.white(),
                            tech_str.white().bold(),
                            content_type,
                            content_length,
                            server
                        );
                    }
                    if sc >= 200 && sc < 300 {
                        // print the final results
                        println!(
                            "{} {} {} {} {} {} {}",
                            domain_result,
                            title.cyan(),
                            status_code.green(),
                            tech_str.white().bold(),
                            content_type,
                            content_length,
                            server
                        );
                    }
                    if sc >= 300 && sc < 400 {
                        // print the final results
                        println!(
                            "{} {} {} {} {} {} {}",
                            domain_result,
                            title.cyan(),
                            status_code.blue(),
                            tech_str.white().bold(),
                            content_type,
                            content_length,
                            server
                        );
                    }
                    if sc >= 400 && sc < 500 {
                        // print the final results
                        println!(
                            "{} {} {} {} {} {} {}",
                            domain_result,
                            title.cyan(),
                            status_code.magenta(),
                            tech_str.white().bold(),
                            content_type,
                            content_length,
                            server
                        );
                    }
                    if sc >= 500 && sc < 600 {
                        // print the final results
                        println!(
                            "{} {} {} {} {} {} {}",
                            domain_result,
                            title.cyan(),
                            status_code.red(),
                            tech_str.white().bold(),
                            content_type,
                            content_length,
                            server
                        );
                    }
                } else {
                    // print the final results
                    println!(
                        "{} {} {} {} {} {} {}",
                        domain_result,
                        title.cyan(),
                        status_code.white(),
                        tech_str.white().bold(),
                        content_type,
                        content_length,
                        server
                    );
                }
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
