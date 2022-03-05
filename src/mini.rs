use slab::Slab;
use std::fmt;
use std::env;
use std::sync::{Arc, Mutex};
use std::net::{SocketAddr};
use hyper:: {Response, Body, Server, Request, Method, Error, StatusCode};
use hyper::service::service_fn;
use futures::{future, Future};
use lazy_static::lazy_static;
use regex::Regex;
use rand;
use pretty_env_logger;
use log::{debug, info, trace}; 

lazy_static! {
    static ref INDEX_PATH: Regex = Regex::new("^/(index\\.html?)?$").unwrap();
    static ref USER_PATH: Regex = Regex::new("^/user/((?P<user_id>\\d+?)/?)?$").unwrap();
    static ref USERS_PATH: Regex = Regex::new("^/users/?$").unwrap();
}

fn mini() {
    pretty_env_logger::init();
    info!("Anatoly's Microservice 1.1");
    let addr: SocketAddr = env::var("ADDRESS")
                            .unwrap_or_else(|_| "127.0.0.1:8080".into())
                            .parse()
                            .expect("can't parse ADDRESS variable");
    debug!("Trying to bind server to address: {}", addr);
    let builder = Server::bind(&addr);
    let udb = Arc::new(Mutex::new(Slab::new()));

    trace!("Creating service handler");
    let server = builder.serve(move || { // ISSUE on .serve
        let user_db = Arc::clone(&udb);
        // clone here because the closure will be called multiple times
        service_fn(move |req| {
            trace!("Recieved Request: {:?}", req);
        microservice_handler(req, &user_db)})
    });
    info!("Used Addr: {}", server.local_addr());
    //let server = server.map_err(drop); //for this need Future
    debug!("Run");
    hyper::rt::run(server); // cannot find function run in hyper rt
}

/// associated type Item and Error not found for futures::Future
fn microservice_handler(req: Request<Body>, user_db: &UserDB) -> impl Future<Item = Response<Body>, Error=Error> {
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

        } 
        // --Long matching sequence here--
        else {
            response_with_code(StatusCode::NOT_FOUND)
        } 
    }; 
    future::ok(response) //Cannot infer type E
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
impl fmt::Display for UserData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("{}")
    }
}
type UserDB = Arc<Mutex<Slab<UserData>>>;