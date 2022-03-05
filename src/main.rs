//C:\rust\hyper_microservice\target\debug\hyper_microservice.exe --help
//cargo watch -x run
//cargo run -- run --address 127.0.0.1:8080
use slab::Slab;
use std::fmt;
use std::env;
use std::io::{Read};
use std::fs::File;
use std::sync::{Arc, Mutex};
use std::net::{SocketAddr};
use std::convert::Infallible;
use hyper:: {Response, Body, Server, Request, Method, Error, StatusCode};
use hyper::service::{service_fn, make_service_fn};
//use futures::{future, Future};
use lazy_static::lazy_static;
use regex::Regex;
use rand;
use pretty_env_logger;
use log::{debug, info, trace, warn}; //internally each of those uses log! which has level arg:
// log!(Level::Error, "Error information: {}", error);
//Level is an enum with variants: Trace, Debug, Info, Warn, Error.
//can do:
//if log_enabled!(Debug) {data=....;debug!("expensive data: {}", data)}
//RUST_LOG=random_service=trace,warn cargo run - filters by warn level and uses trace level for targets starting with random_service prefix
//RUST_LOG_STYLE=auto cargo run - to try using styling
use clap::{crate_authors, crate_description, crate_name, crate_version, Arg, Command};
use serde_derive::Deserialize;

lazy_static! {
    static ref INDEX_PATH: Regex = Regex::new("^/(index\\.html?)?$").unwrap();// matches / , /index.htm , /index.html
    // ^ means must be a string beginning; $ means must be a str ending ; => expect exact matching
    // () implies there must be a group. A group is a unit
    // ? means previous char is optional, and hence second ? means matching just /
    // . fits any symbol, but we need an actual dot so put \. But single \ treated as escape, so need \\.
    static ref USER_PATH: Regex = Regex::new("^/user/((?P<user_id>\\d+?)/?)?$").unwrap();// /user/ , /user/<id> where <id> means group of digits, /user/<id>/
    // ?P sets name of group(regex::Captures accesses the group)
    // \\d matches any digit, \d+ matches any one or more digits, * would be used for 0 repetitions
    static ref USERS_PATH: Regex = Regex::new("^/users/?$").unwrap();// /users , /users/
}

#[tokio::main]
async fn main() {

    dotenv::dotenv().ok(); //reads .env; ok() initialises of it is found
    //In production use actual environment variables, NOT .env 

    let conf = File::open("microservice.toml")
                .and_then(|mut f| {
                    let mut buffer = String::new();
                    f.read_to_string(&mut buffer)?;
                    Ok(buffer)})
                .and_then(|buffer| {
                    toml::from_str::<Config>(&buffer)
                    .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))})
                .map_err(|err| {warn!("Can't read config file: {}", err)})
                .ok();
    
    let matches = Command::new(crate_name!())
                    .version(crate_version!())
                    .author(crate_authors!())
                    .about(crate_description!())
                    .arg(Arg::new("address")
                        .short('a')
                        .long("address")
                        .value_name("ADDRESS")
                        .help("Sets an address")
                        .takes_value(true))
                    .arg(Arg::new("config")
                        .short('c')
                        .long("config")
                        .value_name("FILE")
                        .help("Sets a custom config file")
                        .takes_value(true))
                    .get_matches(); //this reads cmd args with std::env::args_os and matches them
    
    /* 
    For subcommands use something like this:

    let matches = Command::new("Server with keys")
                        .subcommand_required( true)
                        .arg_required_else_help(true)
                        .subcommand(Command::new("run")
                                            .about("run the server")
                                            .arg(Arg::new("address")
                                                    .short('a')
                                                    .long("address")
                                                    .takes_value(true)
                                                    .help("address of the server")))
                        .subcommand(Command::new("key")
                                            .about("generates a secret key for cookies"))
                        .get_matches(); 
    
    
    let x: SocketAddr = match matches.subcommand() {
       Some(("run", sub_m)) => sub_m.value_of("address")
                                                .map( |s| s.to_owned())
                                                .or(env::var("ADDRESS").ok())
                                                .unwrap_or_else(|| "127.0.0.1:8080".into())
                                                .parse()
                                                .expect("can't parse ADDRES variable"),
       _ => "127.0.0.1:8080".parse().expect("can't parse ADDRES variable")
    }; */

    let addr: SocketAddr = matches.value_of("address") // command line arg first
                                .map( |s| s.to_owned())
                                .or(env::var("ADDRESS").ok()) // OR use .env
                                .and_then(|addr| addr.parse().ok())
                                .or(conf.map(|c| c.address)) // OR use config file
                                .or_else(|| Some(([127, 0, 0, 1], 8080).into()))
                                .expect("can't parse ADDRES variable");
                                
    pretty_env_logger::init();
    info!("Anatoly's Microservice 1.1");

    // Builder provides methods to tweak params of server(eg HTTP1 or 2 or both)
    debug!("Trying to bind server to address: {}", addr);
    let builder = Server::bind(&addr);
    let udb = Arc::new(Mutex::new(Slab::new()));
    trace!("Creating service handler");
    let server = builder.serve(make_service_fn( move |_conn| {
        let user_db = Arc::clone(&udb);
        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                //let user_db = user_db.clone();
                trace!("Recieved Request: {:?}", req);
                microservice_handler(req, user_db.clone())
            }))
        }
    }));
    info!("Used Addr: {}", server.local_addr());
    //let server = server.map_err(drop); //for this need Future
    debug!("Run");
    //hyper::rt::run(server);
    let _ = server.await;
}

async fn microservice_handler(req: Request<Body>, user_db: UserDB) -> Result<Response<Body>, Error>{
    let response = { 
        let method = req.method();
        let path = req.uri().path();
        let mut users = user_db.lock().unwrap();

        if INDEX_PATH.is_match(path) {
            if method == &Method::GET {
                let rand_byte = rand::random::<u8>();
                debug!("Generated value is: {}", rand_byte);
                Response::new(INDEX.replace("#RAND#", &*rand_byte.to_string())
                .into())
                //Response::new(Body::from(rand_byte.to_string()))
            } else {
                response_with_code(StatusCode::METHOD_NOT_ALLOWED)
            }

        } else if USERS_PATH.is_match(path) {
            if method == &Method::GET {
                let list = users.iter()
                                .map(|(id, _)| id.to_string())
                                .collect::<Vec<String>>()
                                .join(",");
                Response::new(list.into()) 
            } else {
                response_with_code(StatusCode::METHOD_NOT_ALLOWED)
            }

        } else if let Some(cap) = USER_PATH.captures(path) {
            //Capturing particular group
            let user_id = cap.name("user_id").and_then(|m|
            m.as_str()
             .parse::<UserId>()
             .ok()
             .map(|x| x as usize));

             match (method, user_id) {
                (&Method::DELETE, Some(id)) => {
                   if users.contains(id) {
                       users.remove(id);
                       response_with_code(StatusCode::OK)
                   } else {
                       response_with_code(StatusCode::NOT_FOUND)
                   }
                },
                (&Method::PUT, Some(id)) => {
                    if let Some(user) = users.get_mut(id){
                        *user = UserData;
                        response_with_code(StatusCode::OK)
                    } else {
                        response_with_code(StatusCode::NOT_FOUND)
                    }
               },
                (&Method::GET, Some(id)) => {
                    if let Some(data) = users.get(id) {
                        Response::new(data.to_string().into())
                    } else {
                        response_with_code(StatusCode::NOT_FOUND)
                    }
                },
       
                (&Method::POST, None) => {
                    let id = users.insert(UserData);
                    Response::new(id.to_string().into())
                },
       
                (&Method::POST, Some(_)) => {response_with_code(StatusCode::BAD_REQUEST)},
       
               _ => {response_with_code(StatusCode::METHOD_NOT_ALLOWED)}
               }

        } else {
            response_with_code(StatusCode::NOT_FOUND)
        } 
    }; 
    Ok(response)
}

fn response_with_code(status_code: StatusCode) -> Response<Body> {
    Response::builder()
        .status(status_code)
        .body(Body::empty())
        .unwrap()
}

// # is multiline string
const INDEX: &'static str = r#"
<!doctype html>
<html>
    <head>
        <title>Rust Microservices</title>
    </head>
    <body>
        <h3>Rust Microservices #RAND#</h3>
    </body>
</html>
"#;

type UserId = u64;
#[derive(Debug)]
struct UserData;
//Display auto derives ToString
impl fmt::Display for UserData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        //usually use serde here
        f.write_str("{}")
    }
}
// Slab is an allocator that can store and remove any value identified by an ordered number, and also reuses the slots
// similar to Vec, but doesn't resize if you remove, but wil automatically reuse
// Arc lets us provide references in a shared state
type UserDB = Arc<Mutex<Slab<UserData>>>;

#[derive(Deserialize)]
struct Config {
    address: SocketAddr,
}